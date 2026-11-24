# lazymkv phase 3: unsure discs and failures

Date: 2026-11-24.
Status: the owner answered five questions in the design conversation of 2026-11-24 (Y1 to Y5, part 2); the rest is the designer's, with every call listed in part 16 for the owner to check. The owner approved this document on 2026-11-24. The spec's amendments (part 15) are applied with pull request 1.

"The spec" is `docs/superpowers/specs/2026-10-04-lazymkv-design.md`; "the phase 2 design" is `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md`; "the notes" are `docs/phase2/engine.md`. Y1 to Y5 are the owner's answers of 2026-11-24 and Z1 to Z28 the designer's calls, both recorded here.

## 1. Purpose

Phase 3 is spec section 16, row 3: whole-disc copy mode, the review data model and its commands through `lazymkvctl`, failure classes, waiting states, watchdogs, resume, and the retention cleanup's deletion when space is short.

Why now. Phase 2 ended with an engine that files a disc it is sure about and can do little else. Its acceptance on 2026-11-23 showed what that costs (the notes, "Pull request 7: acceptance"):

- A held job can only be approved as it stands. `fama-05` was held `subtitle_ambiguous` with two full English subtitle tracks, one the film's and one the English commentary's, and both were filed as regular English.
- A job identified at spec 8.1 step 5 has no TMDb record, so it has no final path and cannot be filed whatever is approved.
- A plan in whole-disc copy mode fails with the disc left in.
- The staging data of cancelled, failed and unapproved jobs stays on the share until the owner deletes it by hand (R-A1).
- A rip that hangs before its final message is stopped only by `cancel`, and a waiting job returns only on `retry`.

Phase 3 makes lazymkv finish the discs it is unsure about, with the owner's answer, and cope with what goes wrong without the owner watching.

## 2. The owner's answers of 2026-11-24 (Y1 to Y5)

- **Y1, how a held job is settled.** Both ways: before filing, from a printed review sheet that shows what lazymkv knows; and after filing, by changing a filed job's decisions and filing again while its rip is kept ("fix-after", spec 11's Re-finalise).
- **Y2, how answers are given.** Guided questions in the terminal: each open point is asked in plain words and answered with one letter or Enter. The same answers can be given on one command line, so that they can be scripted or run on the owner's word.
- **Y3, a disc whose title lazymkv cannot pick.** Rip few, copy many: when the candidates are few they are ripped directly and the job is held once, with everything known; whole-disc copy is for a disc with many candidates. This amends spec 8.8's sentence "Title confidence below `auto_threshold` at planning time selects whole-disc copy mode" and the order in D18.
- **Y4, acceptance.** The owner pulls the mini PC's network cable once during an acceptance rip, so a share drop is accepted on a real disc. Failures that cannot be provoked on demand are accepted on stand-in tools only.
- **Y5, the order.** Review first: the review commands, then fix-after, then unsure discs, then failures, then whole-disc copy, then acceptance (part 13).

The owner was told these calls in the conversation and raised no objection; they stay the designer's calls and are listed in part 16: the subtitle lines on the sheet (Z3), the printed path in place of a player (Z4), answers kept as labels (Z7), the limit of 4 (Z10), named cuts in phase 3 (Z11), TV discs held until phase 4 (Z12), whole-disc copy last and behind a trial (Z19), and 30 days of retention (Z18).

## 3. Goals, non-goals and anti-goals

Goals:

- A held job can be settled from the terminal in under a minute by someone who has not read the code: what is open, what lazymkv knows, what the choices are.
- A filed job can be corrected without the disc while its rip is kept.
- An unsure disc still lets the owner walk away (D1): it is ripped or copied, ejected and held.
- No job needs `cancel` or `retry` to get past a hang, a share drop or a full volume.
- The owner can free a job's data with one command.

Non-goals (each has a later home in part 17):

- Assigning TV episodes and filing TV (phase 4). Naming cuts automatically (phase 4).
- The app window, and opening a player for the owner (phase 5).
- New decision rules. Phase 3 changes which mode a plan picks (Y3) and nothing else in section 8's rules. The text rule's tuning on `fama-05` and the audio content rule wait for experiments 13 and 14.
- Evidence from pairing subtitle tracks with audio (later-features item 8) and from menu maps. The review document has room for them (part 6.2); neither is built here.

Anti-goals, which would make phase 3 a failure even if it worked:

- A review that asks the owner what a track is from its packaging, or asks for anything lazymkv could have read itself (D23).
- A prompt that uses the tool's internal words. The owner's feedback of 2026-11-17 on a typed helper was that it was "obtuse and annoying"; every prompt here is a plain sentence that says what to look at, what to type and what Enter does.
- A deletion the owner did not ask for and cannot see in a log.
- Subtitle text written into a job folder, a recording or a log (phase 2 design, part 7.5).

## 4. Constraints

- The repository is public: no disc title, job ID, host, share or user name in it. Slugs only.
- `core` stays pure (RP19): no I/O, tested on recorded inputs on every OS.
- Every transition is persisted before its actions run (spec 10.1).
- The engine never overwrites a library file it did not create in the same job, and deletes nothing inside a library root except what spec 7 names (part 7).
- Nothing is fetched at run time except database answers (D17).
- Over SSH the share is denied; only the engine, in the desktop session, reaches it. `lazymkvctl` therefore reads no share file for a review.
- Builds and tests run on the Mac since 2026-11-21, and Windows is covered by CI; the mini PC runs real discs from a release zip, as pull request 7 did.
- The frozen OCR paths (T5-R54) are not touched.

## 5. Approaches considered

**The structure: one sensible way.** The engine builds one review document per job; `lazymkvctl review` prints it, asks the questions and sends back one set of answers in the form `core` already has (`remembered::Overrides`); the engine saves the answers in the job folder and runs Plan and Decide again with them. Phase 5's app reads the same document. Two alternatives were dropped:

- *`lazymkvctl` builds the sheet from the job's files.* Over SSH the share is denied, and the subtitle text and the rips live there; the app would need the same logic a second time.
- *The engine runs the conversation step by step.* It adds a stateful protocol and half-answered reviews to persist, for no gain over one document and one answer.

**The order: two real choices**, both building the same seven pull requests.

- *Review first* (chosen, Y5): fixes what acceptance hit, the `fama-05` subtitles, the cancelled jobs' data and jobs with no identity. Cost: a hung rip is stopped only by `cancel` for three more pull requests.
- *Failures first*: unattended ripping is safer sooner. Cost: every held disc stays approve-unchanged for one more pull request. The 16 rips so far (14 in the menu work, 2 at acceptance) had no hang and no read error.

**Unsure discs: three choices.**

- *Always copy first* (the spec before Y3): one copy of about 40 minutes whatever the candidates; it needs MakeMKV to scan the copy, which crashed on the one disc tried (EXP-03); the hold after the copy has scan data only; a second hold for tracks can follow.
- *Rip few, copy many* (chosen, Y3): part 8.
- *Never copy*: simplest, but a disc with many decoy titles then blocks the walk-away (D1). Dropped.

What moved the choice: the planner run over the scans of the owner's 40 captured discs on 2026-11-24 (`lazymkv-dev plan` on each `scan.txt` of the menu work's captures; structure only, since 36 of them have no saved database answer, so identity and spec 8.3's rule 4 are left out). 32 plan Direct mode. 8 plan copy mode: 4 that look like TV discs with no episode map, 3 with more than one cut of a film (`walk-03` and `walk-06` with `multiple_features`, `pixelogic-03` with `unnamed_cut`) and 1 with `title_ambiguous` (`ctrm-01`). On 7 of the 8 every candidate is wanted anyway. None has more than 4 film candidates.

## 6. Review

### 6.1 What exists

`core` already holds most of the rules (phase 1): the reducer's rows for approval, discard and re-finalise (`core::job`, `review_line`); `remembered::Overrides` with Set identity, Assign titles, Override tracks and Resolve collision; `plan::plan` and `decision::decide`, which both take overrides; `decision::remember`. The engine uses none of it: `Plan` passes `overrides: None`, `review_action` accepts only `approve` with nothing, and `discard` is refused (`crates/engine/src/owner.rs`, `steps.rs`).

### 6.2 The review document

One pure function in `core` builds a job's review document from its record, its plan, its titles' decisions and what the engine hands in (file paths, subtitle lines, the library's colliding files). The engine sends it as the answer to a new request, `review` (part 11). It holds:

- the job: ID, whether it is held at the gate, held after a copy, or filed, since when, its identity with step and confidence, its notes, and the final path of each title as it stands;
- the points, in spec 8.8's order of reasons. A point has a kind, the review reasons it answers, one plain sentence saying what is open and why, the items to answer, each item's state as it stands and the answers allowed for it, and the evidence for each item;
- every ripped title with its length, chapters, size, staged file path, role as it stands, and its audio and subtitle tracks with what the decision did to each and why.

The sentences are written in `core`, so `lazymkvctl` and the app show the same words (Z1). A held job's document marks as open only the points its reasons raise; a filed job's marks none, and every item can still be changed.

| Kind | Reasons | What the owner answers | Pull request |
|---|---|---|---|
| identity | `identity_unknown`, `identity_ambiguous` | which film this is (part 8.4) | 4 |
| titles | `title_ambiguous`, `multiple_features`, `unnamed_cut` | each title: the film, a named cut, or not wanted (part 8.3) | 4 |
| titles, TV | `tv_unmapped`, or a TV disc with no path | nothing in phase 3: the point says that episodes arrive in phase 4, and only discard is offered (Z12) | 4 |
| audio | `commentary_detected`, `audio_reference_unclear`, `original_language_missing`, `no_wanted_audio` | each track: keep, commentary (dropped) or drop; which is the default | 2 |
| subtitles | `subtitle_ambiguous` | each track: regular, SDH, forced, commentary (dropped) or drop | 2 |
| collision | `path_collision` | keep both, replace the existing file, or discard (part 6.7) | 2 |
| rip | `verify_failed`, `read_errors`, `copy_hash_errors` | accept the rip as it is, or discard and rip again | 2 |

A path two titles of one job share before the owner has given them roles is part of the titles point, never a collision point.

Evidence per item: for a subtitle track that step 10 read, three lines of its text, taken at the same three moments of the title for every track of the point, so that a dialogue track and a commentary track can be told apart at a glance (Z3); its event count; the text rule's verdict and the notes on it. For an audio track: codec, channels, language, the content rule's verdict and measures in one sentence, and its notes. For a title: length, chapters, size and the path to play. For a collision: the path, and the existing file's size and date. An `evidence` list on each item is open-ended, so later evidence (timing pairing, a menu map's label) is added without a format change.

Subtitle lines are the only subtitle text that leaves the engine. They are read from the staged OCR reading, or from `tuning_root`'s dataset once the staged copy is gone, put in the document, sent over the pipe and printed. They are never written to the job folder, a recording, `job.log`, `engine.log` or a push message (Z3). A track that was not read shows no lines and says why.

### 6.3 The sheet and the guided questions (Y2)

`lazymkvctl review JOB` asks for the document, prints the sheet and, when its input is a terminal, asks each open point. For `fama-05` (the lines of text are made up, and the slug stands where a real job ID, a fingerprint and a number, goes):

```
$ lazymkvctl review fama-05

Held since 23 Nov. Identified by TMDb (0.90). One point needs your answer.

1. Subtitles: two full English tracks that do not match each other.
   lazymkv cannot tell which one carries the film's dialogue.

   track 3  English  1,224 lines   "Where are you going?" / "[door slams]" / "MARIA: Wait."
   track 4  English  1,410 lines   "Hi, I'm the director." / "We shot this scene in March."
   To watch first: <path of the ripped file on the share>

   As it stands: both kept as regular English.
   track 3 is:  r regular   s SDH   f forced   c commentary (dropped)   d drop     [Enter = r]
   track 4 is:  r regular   s SDH   f forced   c commentary (dropped)   d drop     [Enter = r]

Files as: Movies\<Title (Year)>\<Title (Year)> - 2160p.mkv
File it now?  y yes   n not yet   x discard the job
```

Rules of the dialogue:

- Enter always keeps an item as it stands. An answer that is not one of the letters shown is asked again with the same line.
- After the last point the sheet shows what would be filed, with the changes, and asks once: file it, not yet, or discard. "Not yet" sends nothing and saves nothing; the job stays as it was (Z2).
- At the end `lazymkvctl` prints the one-line form of the answers just given, so a review can be repeated or handed over.
- `review JOB --answer ... --yes` gives the same answers without questions. Without a terminal and without `--answer`, `review` only prints the sheet. `--all` asks every item of a held job, not only the open points. `--json` prints the document itself.
- `approve JOB` stays: it accepts a held job as it stands.
- Wording comes before code is called done: before pull request 2 is finished, the owner is shown the sheet of one real job and the wording is corrected from that reading (the owner's feedback of 2026-11-17: try a procedure on one real item first).

Watching before filing: the sheet prints the staged file's path, and the owner opens it in any player on any machine that reaches the share. Spec 11's Preview, which opens a player, waits for the app (Z4).

### 6.4 Answers

The answers of one review are one `Overrides` value (`identity`, `season`, `titles`, `audio`, `subtitles`, `collision`), sent with `review_action` (part 11). The engine checks them against the job before it accepts them: every title and stream named exists in the job's scan; a role is one phase 3 files (the film, a named cut, not wanted; an episode role is refused, naming phase 4); a title given a role was ripped or, after a copy, passes the candidate filter; at least one title is wanted; a named cut has a name; `replace` names a path the document showed as colliding.

The engine keeps the job's answers in the job folder as `review.json`: the merged `Overrides` and, for each review, its time, its action and the reasons shown. A later review's answer for the same title, stream or path replaces the earlier one; answers it does not touch stay (Z5). The relayed `plan --job` and `decide --job` hand the same overrides to the `dev` library (a new `--overrides FILE`), so `compare-job` still compares like with like.

"Commentary" on a subtitle track is sent as the override Drop, and on an audio track as Commentary; both also carry a label (part 6.6).

### 6.5 What an approval does

At the disposition gate the engine delivers `ReviewApproved` with the merged overrides and the reasons the document showed, once the file-phase slot is free (as phase 2 does for an unchanged approval). The reducer answers with Decide. The engine's Decide in a job that has answers then runs in this order:

1. Take the reviewed identity's TMDb record, which was fetched and saved when the answer was given (part 8.4), so this step fetches nothing new.
2. Plan again with the overrides and any remembered decision. An identity in `review.json` wins over whatever the lookup book gives, here and wherever the plan's identity is refreshed. A title's key never changes after its rip: the plan made again keeps each ripped title's key, record and files, and changes only roles, identity and confidences.
3. Decide every title that is wanted, with the overrides. A title that is not wanted gets no decision, no remux and no path; its rip stays with the job's staging data.
4. Feed `DecisionDone`. The reducer files the job, or holds it again when the decision raises a reason the owner was not shown, `path_collision` without an answer, or no final path (`blocks_filing`, unchanged).

After a restart the engine reads `review.json` back with the rest of the decision's inputs (phase 2 design, part 5.10).

### 6.6 Remember, and labels

**Remember is automatic (Z6).** Every approval or fix-after that carries an answer writes `%ProgramData%\lazymkv\remembered\<fingerprint>.json` through `decision::remember`, once the job is filed. A later job for the same disc then applies the answers and files without a hold. A remembered file that cannot be written is logged in `job.log` and never fails the job. This amends spec 11, where Remember is an action of its own.

**Labels (D16, spec 7).** An answer that says what a track is (regular, SDH, forced or commentary for subtitles; commentary for audio) is kept as a label in a file of its own in the job folder, `review-labels.json`, which the tuning refresh copies with the recording as it copies every file of the job folder. Each entry holds the time, the title key, the stream, the label, and the verdict the sheet showed, which is the label it replaced. The owner's own label files, `audio-labels.json` and `subtitle-labels.json`, are never written by the engine, as today (T5-R15). A review label never counts towards spec 17's rows 13 and 14, because the sheet shows lazymkv's verdict before the owner answers; `retune` does not read the new file in phase 3 (Z7). "Keep" and "drop" say what the owner wants, not what the track is, and write no label.

### 6.7 Collisions

A `path_collision` point offers spec 11's three answers. "Keep both" files under the next free copy label (`- 2160p 2`). "Replace" asks the owner to type the word `replace`; the engine then moves the existing file into `<staging>\<job id>\replaced\` before filing. That file is kept for `retention_days` from the day it was replaced and is deleted only by age: deletion when space is short (part 9.5) never takes it (Z27). "Discard" is part 6.8.

Two cases that phase 2 fails as `Unknown` become holds with `path_collision`, as the phase 2 design said they would (its parts 6.5 and 14.1):

- LIB-8: a file appears at the final path between the decision and the rename. A new reducer row takes `Verified` to the gate (part 12).
- L7: the share says the final folder exists and the engine cannot open it. The point says that this clears by itself within minutes and offers one answer, try again.

A job held in either way has already written and verified its partial file. The new row deletes that partial file, and the approval remuxes again, as every approval does. That is about half an hour of the machine's time on a UHD and none of the owner's, in a case that should be rare; filing the kept partial file would need a second way out of the gate in the reducer (Z25).

### 6.8 Discard

`lazymkvctl discard JOB` asks the owner to type the word `discard`, naming the job and the size of what goes; `--yes` skips the question. `discard` and `replace` are the only two answers that take more than a letter, because neither can be undone (Z21). The engine accepts `discard` for a held job, a failed job of any class and a waiting job (Z8; the reducer has the first two, part 12 adds the third). For a job with a step running it is refused, and the reply names `cancel`, which fails the job so that it can then be discarded.

`Discarded` is persisted first. The job's staging folder `<staging>\<job id>\` and its partial files are then deleted by the cleanup task, which gains one rule: a discarded job's staging data is deleted at any pass, the first of which runs at once (Z9). One deleter keeps phase 2's guards: only a folder shaped like a job ID, no link followed, nothing if the share does not answer. A pending tuning copy of a discarded job is dropped, and its `tuning_copy` says so. The job folder under `%ProgramData%\lazymkv\jobs\` stays, so history shows the job. Nothing in a library root and nothing under `tuning_root` is deleted.

Discard never touches a filed job. Its file stays in the library, and its rip goes by retention.

## 7. Fix-after (Y1)

`lazymkvctl review JOB` on a filed job shows the same sheet with no open point and every item changeable, then asks whether to file again with the changes. The engine accepts it while the rip is there: the job is `Filed` and every wanted title's staged file exists. Otherwise it refuses, saying when the rip was deleted and that `rip-again` is the way.

The engine delivers `ReFinalise` with the filed decision's reasons, once the file-phase slot is free. Decide runs as in part 6.5, then the remux and its verification as in any job. Filing follows spec 7's "Overwriting":

- The same path: the new file is written and verified as a partial file, then replaces the job's own file, and only if that file's size and modification time still match the job's record. A file that no longer matches is left alone, and the job is held with `path_collision`.
- Another path (a changed identity or edition): the new file is filed first; the job's old file is then deleted if it still matches its record, and otherwise left, with the note `old_file_kept`.
- A title that is no longer wanted: its filed file is deleted under the same match rule.

For this the job's own filed files must not count against it. Wherever the library is read for a job's decision, a file at a path the job's record holds, with the recorded size and modification time, is left out: it is no collision, and it is not an earlier copy for spec 8.4's edition names. A file there that does not match the record counts as any other file. `core`'s `LibraryFacts` does not change (Z26).

The job is `Filed` again, `filed` is sent again, the record holds the new file's path, size and time, and the retention count starts again from the new filing (Z17). The answers are remembered and labelled as in part 6.6.

Fix-after changes what a title's file holds and how it is named. It cannot add a title that was not ripped; that is `rip-again` (spec 8.8, "Limits of D1").

## 8. Unsure discs (Y3)

### 8.1 Which mode

A plan's mode today is `plan::mode(title_confidence, settings)`: Direct at or above `auto_threshold`, else Copy. Phase 3 makes it:

1. Title confidence at or above `auto_threshold`: Direct, as now.
2. Below it, a TV disc: Direct, for every planned episode-length title. Every one of them is wanted, so a copy would only add a second read (Z12).
3. Below it, a film whose candidate set has between 1 and `direct_candidates_max` titles (a new setting, default 4): Direct, for the whole candidate set (Z10).
4. Otherwise: Copy (part 10).

The candidate set of a film is every title the plan lists, plus, for a rival group in which spec 8.3's rule 3.4 could not pick a title, that group's other titles. The order is the plan's, most likely first, each group's other titles after its pick. The space check is the existing one over the planned titles (spec 7).

### 8.2 What the job does

The candidates are ripped and verified as any Direct job's titles, the disc is ejected and `disc_done` is sent. Every candidate is then probed, read and decided, and the job is held once at the disposition gate with the plan's reasons and the tracks' reasons together. The owner answers the titles point and any track point in one sitting, with files to play.

Cost, accepted with Y3: each candidate is a full rip. Two cuts of a UHD are about two hours in the drive and 100 GB or more on staging, against one copy of about 40 minutes and the disc's size. Both cuts would be extracted from a copy anyway (D2).

### 8.3 Assigning titles

Each ripped title is one of: the film; a named cut, with an edition name of at most 32 characters; not wanted. `core` already files a named cut as an edition (spec 8.4, 8.9), so the review offers it now; naming cuts without the owner stays in phase 4 (Z11). Spec 8.4's rules decide whether the main cut is named "Theatrical Cut".

A title marked not wanted keeps its rip until the job's staging data goes, so fix-after can still bring it back.

### 8.4 Setting the identity

The identity point lists the plausible TMDb results the job saved, numbered, each with its kind, title and year. The owner picks a number, or types a name to search again, or a TMDb ID with its kind. A new request, `identity_search`, runs one TMDb search of both kinds through the engine, which holds the token, and returns the results without saving anything. The chosen identity goes into the answers. When the engine receives them it fetches that identity's record before it accepts the approval, and saves it with the job. If TMDb does not answer, the approval is refused with that reason and nothing changes, so the owner, who is at the terminal, tries again; no job waits on a lookup for an identity it was given (Z22). The lookup chain carries the reviewed identity from then on (the notes' "the reviewed identity (spec 11) carried in `ChainInputs`"). Identity confidence becomes 1.00 (spec 8.8).

This is what resolves a job identified at spec 8.1 step 5 with no record, and a DVD whose name is generic.

### 8.5 TV discs in phase 3

An unmapped TV disc is ripped title by title and held with `tv_unmapped`; a mapped one is held with no path, as today. Neither can be filed before phase 4, when `tv_root` and the episode roles arrive. The sheet says so and offers only discard. The rips stay, as any held job's data does (spec 7), so phase 4's review needs no disc (Z12).

## 9. Failures, watchdogs, waits and retention

### 9.1 Failure classes

Spec 10.2's table stands. A table in `core` maps MakeMKV's message codes to classes and holds only codes a recording shows; today those are the licence messages. `DriveIncapable`, `NeedsNetwork` and `NeedsMakeMkvUpdate` stay `Unknown` until a message is captured, as spec 10.2 says; a first sighting adds a row and a recording, with no other change (Z13).

### 9.2 Watchdogs (spec 10.4)

- **A rip or a copy that stops making progress.** Progress is a `PRGV` line whose values moved, as spec 10.4 has it. After `stall_timeout_s` (300) without progress the engine checks the staging share. Unreachable: the attempt is ended and classed `Output`, so the job waits in AwaitingShare. Reachable: the job is marked "drive struggling" and the run goes on, since MakeMKV's own retries decide when a title fails.
- **A rip or a copy that goes silent.** Silence is no output line of any kind, `PRGV` or `MSG`, and every line starts the count again. After `exit_wait_min` (30 minutes) of silence the engine kills the process tree, waits 10 s, does not eject, runs the share check and fails the job as `MediaReadError` with "process did not exit". A run that keeps printing read errors is not silent, and this rule never kills it: MakeMKV's own retries end it, or the owner's `cancel`. Phase 2 applies the exit wait only after the final message; this reads spec 10.4's sentence on a blocked read as covering a run that never gets there (Z14).
- **Read errors.** More than `read_error_rate` (20) `MSG:2003` lines in a minute marks the drive as struggling. Nothing is killed.
- **A silent scan.** No line for `stall_timeout_s` ends the scan; it is retried once, the same one retry as the scan past `scan_timeout_min`.
- **`mkvmerge`.** No `#GUI#progress` line for `stall_timeout_s` ends a remux or an extraction; the share check then classes it `Output` or `Unknown`. The phase 2 design left this rule open until progress lines were seen through a pipe (its part 5.11). They are: in `fama-05`'s job the remux printed 101 progress lines over 2,032 s with a longest gap of 21.6 s, and the subtitle extraction 83 over 1,040 s with a longest gap of 48.3 s and the first after 43.9 s (the job's `remux-mkvmerge.txt.timing.jsonl` and `evidence-mkvmerge.txt.timing.jsonl`). The rule is built as spec 10.4 words it.

"Drive struggling" is a field of the job summary, shown by `status` and `watch`, cleared when the step ends. It sends no notification (Z15).

### 9.3 Share loss in any step

Phase 2 classes only a rip by the share check; a share lost during Decide, Remux, the file verification or File fails the job as `Unknown` (V7). In phase 3 every step that finds a root it needs unreachable, before it starts or as the cause of a tool's failure, feeds `ShareLost` or a failure of class `Output`, and the job waits in AwaitingShare. The reducer already takes both in every working state.

Every share call (reachability, free space, a listing) gets a time bound of 30 s; a call that does not answer counts as unreachable (Z16).

### 9.4 Waiting states that end by themselves

Phase 2 leaves a waiting state only on what spec 10.1 names or on `retry`. Phase 3 adds engine-side checks, with no reducer change:

| State | Checked | Returns with |
|---|---|---|
| AwaitingShare | every 60 s: the roots the job needs | `ShareBack` |
| AwaitingSpace | after every cleanup pass, and every 10 minutes | `SpaceFreed` |
| AwaitingTool, AwaitingMakeMkv | hourly, beside the checks at engine start, at an insertion and at a self-test | `ToolFound`, `MakeMkvChanged` |
| AwaitingLookup, AwaitingNetwork | their timers, as now | as now |

`retry` still checks at once. A return to the disc phase with the disc still in the drive runs the interrupted step again from its start (spec 10.1: the title's folder is emptied and the title ripped again), so a share that drops mid-rip costs one more read of that title and no action from the owner. A job is also held in AwaitingTool for a tool below its minimum version, which phase 2 only reports (the notes, "Phase 3").

### 9.5 Deletion when space is short (spec 7, P4)

When a space check is short on a volume, the engine first asks the cleanup task for room: it deletes the raw data of `Filed` jobs on that volume, oldest filing first, whatever their age, until the check passes or none is left. It never deletes the data of a held, failed, waiting or running job, nor of a filed job whose tuning copy is pending, nor a library file that "replace" moved into a job's `replaced\` folder (part 6.7). Only then is `SpaceShort` fed, and a job waiting in AwaitingSpace gets the same pass again at each check of part 9.4. `job.log` and `engine.log` name every job whose rip went early and the job it went for, and the `waiting` notification says how much room is missing.

A rip deleted this way ends that job's fix-after early. That is the price of Y1's long window: the owner's `retention_days` is set to 30 at acceptance (Z18), the default stays 7, and this rule is what makes a long window safe.

### 9.6 Resume

`core::job::resume` stands. Phase 3 adds its rows for the new states (Copying, Extracting, held after a copy) to the engine, and the returns of part 9.4. A job that failed with `MediaReadError` resumes when its disc goes into either drive, as now, and rips only the titles not yet verified.

## 10. Whole-disc copy (spec 6.3, D18)

Copy mode serves a film with more candidates than `direct_candidates_max`. None of the owner's 40 captured discs is one, so it is the last piece built, and it starts with a trial (Z19).

**The trial**, before any copy-mode code: on two of the owner's UHDs, `backup --decrypt` to staging, `info file:` on the copy with the scan's flags, and `mkv file:` for one title, whose output is verified against the disc scan as a rip is. It answers what EXP-03 left open: whether MakeMKV 2.0.0 scans a copy at all (it crashed on one playlist of one disc), and whether a title extracted from a copy equals a rip. If the copy's scan fails on both discs, the result goes to the owner before anything else is built; ripping the most likely candidates directly would then be all that copy mode can do.

**The build**, if the trial passes, is spec 6.3 as written:

- `CopyDisc`: `backup --decrypt disc:N <staging>\<job id>\backup\`; success is `MSG:5070` with no `MSG:5069`; `MSG:5079` keeps the copy and adds `copy_hash_errors`. Then the scan of the copy, while the disc is in.
- The copy's scan lists titles: eject, `disc_done`, and the job is held after the copy. Its sheet has scan data only: each candidate's length, chapters, size and segments, and the path of its largest stream file in the copy to play. After the owner assigns titles, each is matched between the two scans by source file, segment map and duration, extracted, verified, and the job goes on to the file phase, where a second hold for tracks can follow.
- The copy's scan fails, or the copy fails a second time on a read error: Direct mode with the note `direct_mode_fallback`, for the `direct_candidates_max` most likely candidates, not for every title that passes the candidate filter (Z20, which amends D18's fallback: with many candidates the old wording would fill staging and hold the drive).
- A plan with no candidate at all (Z10) is copied like any other. The hold after its copy offers every title the copy's scan lists. If its copy cannot be scanned there is nothing to fall back to, and the job fails as `Unknown` with the copy kept and the disc left in (Z28).
- `cancel` gains its rows for Copying and Extracting (the notes, "The pull request that builds copy mode"; R-A7's table).

Until this pull request lands, a plan in copy mode fails with the disc left in, as in phase 2.

## 11. IPC and lazymkvctl

The protocol stays at `v` 1; an older client ignores the new fields and gets an error for no request it knows.

| Request | Payload | Answer |
|---|---|---|
| `review` (new) | `job`, `all` | the review document (part 6.2) |
| `identity_search` (new) | `job`, `query` | TMDb results: kind, ID, title, year |
| `review_action` | `job`, `action` (`approve` or `refinalise`), `overrides`, `labels` | `ok`, or an error naming the first answer that does not fit the job |
| `discard` | `job` | accepted for a held, failed or waiting job |
| `secret_unset` (new) | `name` | removes a stored secret (the notes, "Phase 3") |

The job summary gains `struggling`. No push carries subtitle text. Two items the notes list under "Phase 3" are not built: cancelling lazymkv's own share connections (`WNetCancelConnection2W`) and listing stored share names (Z23).

| Command | What it does |
|---|---|
| `review JOB [--all] [--json] [--answer TEXT ...] [--yes]` | part 6.3; on a filed job, part 7 |
| `approve JOB` | accepts a held job as it stands, as now |
| `discard JOB [--yes]` | part 6.8 |
| `secret unset NAME` | `secret_unset` |
| `plan`, `decide` | gain `--overrides FILE`; the relayed forms pass the job's `review.json` |

Exit codes follow the phase 2 design's part 9.6. `review` exits 0 when the job was filed, held again or left as it was on "not yet", 1 when the engine refused an answer, 2 on an argument error.

## 12. Changes in core

- `plan::mode` takes the plan's kind and candidate set (part 8.1); `Settings` gains `direct_candidates_max`.
- The reducer gains two kinds of row and nothing else: `ReviewDiscarded` from every waiting state, to `Discarded` (Z8); and a file collision at `Verified`, to the gate with `path_collision`, deleting the partial file (part 6.7). Every new row has its test, as spec 15 requires.
- A `review` document type and its builder (part 6.2), with the sentences.
- The IPC types of part 11.
- The message-code table of part 9.1.

No rule of spec section 8 changes.

## 13. The pull requests (Y5)

One implementation plan, its tasks grouped into pull requests in this order. Each branches from `main` after the one before merges, ends with whole jobs on stand-ins and green CI on the three systems, and lists any new crate for approval before it is fetched.

1. **Design.** This document and the spec's amendments (part 15).
2. **Review.** The document and its builder; `lazymkvctl review` with the guided questions and the one-line form; approval with track answers and collision answers; `review.json`; Decide with overrides; remember; labels; discard and the cleanup's rule for it; the LIB-8 and L7 holds. Ends with: a stand-in job held with two English subtitle tracks is reviewed and filed with one; the owner has read one real job's sheet.
3. **Fix-after.** `review` on a filed job, `ReFinalise`, and filing by spec 7's replace rules. Ends with: a filed stand-in job is changed and its file replaced; a file that no longer matches its record is left alone.
4. **Unsure discs.** The mode rule and its setting; ripping a candidate set; the titles point; the identity point, `identity_search` and the reviewed identity in the lookup chain. Ends with: a stand-in disc with two rivals is ripped, held once, assigned and filed as two editions; a job with no identity is given one and filed.
5. **Failures.** The watchdogs, share loss in any step, the share call bound, the waiting states' checks, the tool-minimum hold, deletion when space is short, `secret unset`, and the stand-ins' new faults (part 14). Ends with: stand-in jobs that stall, lose the share, run out of space and hit read errors each end where spec 10 says, with no command given.
6. **Whole-disc copy.** The trial first, then part 10.
7. **Acceptance.** Part 14.2, its results and the spec facts it verifies, written into `docs/phase3/` notes.

## 14. Testing and acceptance

### 14.1 Tests

- **`core`:** the mode rule on recorded scans; every new reducer row; the review document as goldens on the recordings and on hand-made scenarios, one per point kind; the labels' new value.
- **Whole jobs on stand-ins** (every CI system): each pull request's ending above. The stand-ins gain what phase 3's code reacts to (the phase 2 design, part 12.2, left these for now): `MSG:2003` read errors with a failed title; silence until killed, for `makemkvcon` and `mkvmerge`; a write error (`MSG:2019`); `backup`, `info file:` and `mkv file:` runs with `5070`, `5069` and `5079`; a copy scan that ends without `MSG:5011`. The fake share connector gains a share that stops answering mid-step and comes back, a call that never returns, and free space that changes.
- **`lazymkvctl review`:** scripted input for every point kind, the one-line form giving the same request as the guided answers, and no subtitle line in anything but standard output.
- **Real tools on small files:** fix-after's replace on generated files.

### 14.2 Acceptance on real discs

On the mini PC, from a release zip, filing into the test library, as pull request 7 did. `retention_days` is 30.

1. **`fama-05` again** (with `rip-again`). It is held; the sheet shows both English subtitle tracks with their lines; the owner marks the commentary track; it files with one English subtitle track, and the label is written. Its earlier file is still in the test library, so the job also meets `path_collision`, and the owner answers "replace". The owner checks Jellyfin. Then one fix-after change on the filed job, the file is replaced in place, `filed` arrives again, and the owner checks Jellyfin again.
2. **A disc with two cuts** (`walk-03` or `walk-06`). Both cuts are ripped, the job is held once, the owner names them, both file, and Jellyfin shows two versions of one film. The planner was run on these discs from structure alone; if the disc plans otherwise with its database answers, another unsure disc is taken and the difference recorded.
3. **Discard.** The two cancelled jobs that phase 2's acceptance left are discarded, their staging folders are gone and history shows them.
4. **A share drop (Y4).** During one of the rips above, the owner pulls the mini PC's network cable for about ten minutes. The job must wait in AwaitingShare, return by itself once the cable is back, rip that title again and finish. What `makemkvcon` printed is recorded; it is the share-drop half of experiment 4, which was never run.
5. **Whole-disc copy**, if pull request 6 built it: one job forced into copy mode by `direct_candidates_max` 0, from copy to filed.

It passes when every item ends as described, `compare-job` finds no difference on each filed job's own recording with its answers, every notification arrives, and nothing was written outside the engine's staging and tuning folders and the test library.

Read errors, stalls, a hung `makemkvcon` and deletion when space is short cannot be provoked on demand. They are accepted on stand-ins, and the first real one is recorded when it happens.

## 15. Spec amendments, applied with pull request 1

| Where | Change |
|---|---|
| 4.5 | add `review`, `identity_search` and `secret_unset`; `review_action` takes `approve` and `refinalise` with overrides and labels; `discard` is accepted |
| 6.3, first paragraph; 8.8, "Title confidence below..."; 10.1's row for `Planned` | the mode rule of part 8.1 (Y3) |
| 6.3, the two fallback sentences; D18; section 17 row 3 | the fallback rips the `direct_candidates_max` most likely candidates (Z20) |
| 7, "Retention" | deletion when space is short as part 9.5 words it |
| 10.1's table | discard from a waiting state; the collision row from `Verified` |
| 10.4 | the silent-run reading of part 9.2 (Z14); the measured `mkvmerge` progress |
| 11 | Remember is automatic (Z6); Preview is a printed path until phase 5 (Z4); the actions are reached through `review` |
| 14 | `direct_candidates_max`, default 4 |
| 16, row 3 | points to this document; acceptance as part 14.2 |

## 16. The designer's calls, for the owner to check

- **Z1.** The sheet's sentences are written in `core`, so the terminal and the app show the same words.
- **Z2.** "Not yet" saves nothing. Answers are sent only when the owner files or re-files.
- **Z3.** Three lines of subtitle text per track are shown on the sheet, at the same moments for every track of a point. They go to the screen only.
- **Z4.** Watching before filing is a printed path; no player is opened until phase 5.
- **Z5.** Answers of successive reviews of one job add up; a later answer replaces an earlier one for the same thing.
- **Z6.** Remember is automatic on every approval or fix-after with an answer.
- **Z7.** A review answer that says what a track is is kept as a label in the job's own `review-labels.json`. It never counts towards experiments 13 and 14, and the engine still never writes the owner's two label files.
- **Z8.** A waiting job can be discarded, besides a held and a failed one.
- **Z9.** Discard marks the job, and the cleanup task deletes its data, at once and again at each pass until it is gone.
- **Z10.** The limit is 4 film candidates, as the setting `direct_candidates_max`. A plan with no candidate at all is copy mode.
- **Z11.** The review offers "a named cut" in phase 3, since `core` files it already.
- **Z12.** A TV disc below the threshold is ripped title by title and held until phase 4, with only discard offered.
- **Z13.** `DriveIncapable`, `NeedsNetwork` and `NeedsMakeMkvUpdate` stay `Unknown` until a message is captured.
- **Z14.** A rip or copy that prints no line of any kind for `exit_wait_min` is killed and failed as `MediaReadError`, with the share healthy. A run that keeps printing read errors is never killed by lazymkv. Spec 10.4 can also be read as never killing a silent run.
- **Z15.** "Drive struggling" is shown in `status` and sends no notification.
- **Z16.** A share call is bounded at 30 s, and the waiting states are checked every 60 s, 10 minutes and hour as part 9.4 lists.
- **Z17.** Filing again restarts the retention count.
- **Z18.** The owner's `retention_days` becomes 30 at acceptance; the default stays 7. The owner was asked and gave no answer, so this is the designer's.
- **Z19.** Whole-disc copy is the last piece and starts with a trial on two discs; a failed trial comes back to the owner.
- **Z20.** Copy mode's fallback rips at most `direct_candidates_max` titles.
- **Z21.** Typing the word is required for `discard` and for `replace`; nothing else asks for more than a letter.
- **Z22.** The record of an identity the owner picks is fetched when the answer is given; if TMDb does not answer, the approval is refused and nothing changes.
- **Z23.** `WNetCancelConnection2W` and listing stored share names, both under "Phase 3" in the notes, are not built: nothing in the owner's setup uses lazymkv's stored share credential.
- **Z24.** Acceptance item 1 uses the collision with `fama-05`'s earlier file to accept "replace", instead of clearing the test library first.
- **Z25.** A job held for a collision found at filing remuxes again after the answer; its verified partial file is not kept.
- **Z26.** A job's own filed files, matching its record, are left out of what is read of the library for its decision. This is what lets fix-after replace its own file.
- **Z27.** A library file that "replace" moved aside is kept for `retention_days` from that day and is never deleted for space.
- **Z28.** A copy-mode job with no candidate offers every title of the copy's scan; if that scan fails, the job fails with the copy kept.

## 17. Left to the builder, and left for later

Left to the builder: the document's field names and the answer grammar of `--answer`, fixed as serde types and tests in pull request 2; which three moments the subtitle lines are taken at; how a ripped title keeps its key through a plan made again; whether `ffmpeg`'s level pass can take the silence rule or needs a ceiling; the layout of `review.json`.

Left for later:

- Phase 4: episode roles and TV filing; naming cuts automatically; A2's TV half.
- Phase 5: the app's review screen over the same document; Preview that opens a player.
- No phase yet: timing pairing and menu-map labels as evidence on the sheet (spec 16's later-features item 8; the menu maps work); answers saved between sittings; forcing copy mode for one disc; the message texts of the three uncaptured failure classes; the text rule's tuning on `fama-05`.
