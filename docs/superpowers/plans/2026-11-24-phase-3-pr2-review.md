# Phase 3 Pull Requests 1 and 2: Design and Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A job held for its tracks or for a path collision can be settled from the terminal with `lazymkvctl review`, a job's data can be freed with `lazymkvctl discard`, and the owner's answers are kept, remembered and replayed.

**Architecture:** `core` builds one review document per job from the job's record and renders it as a sheet; the engine fills in what only it can read (subtitle lines, the colliding file) on a worker thread and answers the new `review` request with it. `lazymkvctl review` prints the sheet, asks the open points and sends one `review_action` whose answers are `core`'s existing `remembered::Overrides`. The engine keeps them in the job folder as `review.json` and hands them to Decide.

**Tech Stack:** Rust (the workspace toolchain, `serde`, `serde_json`), the stand-in tools of `crates/fake-tools`, GitHub Actions (`ci.yml`).

**Spec:** `docs/superpowers/specs/2026-11-24-phase-3-design.md` ("the design"; its parts 6, 11, 12 and 13 items 1 and 2). Binding above it: `docs/superpowers/specs/2026-10-04-lazymkv-design.md` ("the spec") sections 4.5, 7, 8.8, 10.1 and 11, and `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` ("the phase 2 design") parts 5.10, 7.5 and 9.

This plan covers pull requests 1 and 2 of the design's part 13. Pull requests 3 to 7 each get their own plan after the one before has merged, as phase 2's did.

## Global Constraints

- Rust is built and tested on the Mac and by CI; nothing is built on the mini PC. The Mac's command prefix, written `CARGO` below:
  `env RUSTUP_AUTO_INSTALL=0 PATH="$HOME/.cargo/bin:$PATH" LAZYMKV_OCRS_MODELS="$HOME/.cargo/lazymkv-ocrs-models" cargo`
  The whole run is `CARGO test --workspace --exclude lazymkv-ocr --locked --offline`. No new crate: `Cargo.lock` does not change.
- Commits are signed, as `krcm0209 <17315774+krcm0209@users.noreply.github.com>`, with no attribution line. The controller makes every commit; an implementer never commits. After each commit: `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` shows `G`, that identity and no agent line. One commit per task.
- Nothing is pushed and no pull request is opened without the owner's word (prerequisites 1, 2 and 5).
- No title of the owner's disc, job ID, host, share, user name or drive serial in the repository or the ledger. `scripts/phase0/check-names.sh` exits 0 before every commit.
- No subtitle text in any file the engine or a test writes into a job folder, a recording, `job.log`, `engine.log` or a push message. Test fixtures use made-up lines.
- `core` stays pure: no file, clock or process access (RP19). A share is never read on the engine's owner thread; share reads run on a step, check or worker thread.
- Every transition is persisted before its actions run (spec 10.1). The protocol stays at `v` 1.
- The frozen OCR paths (`crates/core/src/subtitle/bitmap/`, T5-R54) are not touched.
- The engine never writes `audio-labels.json` or `subtitle-labels.json` (T5-R15).
- Rulings made while executing are labelled T32-E1, T32-E2 and so on in the ledger `.superpowers/sdd/2026-11-24-phase-3-pr2-review/progress.md` (git-ignored) and carried into the notes by Task 18. This plan's own rulings are T32-R1 to T32-R9.

## Owner prerequisites and decisions

| # | What | When |
|---|---|---|
| 1 | The word to push pull request 1's branch (the design, the spec's amendments, this plan), open it, and merge it | after Task 1 |
| 2 | The word to push pull request 2's branch and open it as a draft | end of Task 16 |
| 3 | Yes or no to copying the two OCR text files of the `fama-05` job from the tuning dataset into the Mac's git-ignored `.superpowers/pr32/` for the wording trial. Without it the trial's sheet shows no lines | start of Task 17 |
| 4 | Reading the real sheet and saying what is unclear | Task 17 |
| 5 | Marking pull request 2 ready for review | end of Task 18 |

## This plan's rulings

- **T32-R1 (what pull request 2 answers).** Track answers and collision answers only. An `overrides` value that holds `identity`, `season` or `titles` is refused with `this version does not take <key> answers yet`; they arrive with pull request 4. The sheet still shows the identity and titles points, marked not answerable.
- **T32-R2 (which tracks are items).** Every kept audio and subtitle track of a title, and every dropped one whose reason the owner can overrule: audio `flagged`, `secondary`, `duplicate`, `described`, `remembered`, `reviewed`; subtitles `directors_comments`, `outside_dialogue_group`, `partial`, `extra_dialogue_track`, `extra_forced`, `closed_captions_unused`, `remembered`, `reviewed`. A track dropped as `derived`, `unwanted_language`, `other_language`, `derived_forced_only_empty`, `derived_forced_only_unused` or `no_events` is never an item.
- **T32-R3 (item ids and answer words).** An item's id is `<title key> audio <stream>`, `<title key> sub <stream>` or `<title key> collision`. Words: audio `keep`, `default` (keep, and play by default), `commentary`, `drop`; subtitles `regular`, `sdh`, `forced`, `commentary`, `drop`; collision `keep_both`, `replace`. Letters on the sheet: `k`, `p`, `c`, `d`; `r`, `s`, `f`, `c`, `d`; `k`, `r`. `--answer` takes `<id>=<word>`, and the title key may be left out when the job has one title.
- **T32-R4 (the rip point).** `verify_failed` and `read_errors` give a point with no item: filing accepts the rip as it is, and the other way is discard and `rip-again`. `original_language_missing` and `no_wanted_audio` head the audio point.
- **T32-R5 (the three moments).** Subtitle lines are taken at 25%, 50% and 75% of the title's length: for each track the first event with text at or after that moment. A line is cut to 60 characters, control characters removed.
- **T32-R6 (`review` returns when the engine accepts).** `lazymkvctl review` exits 0 once the engine has replied `ok` to the answers, and says how to watch the job; it does not wait for the remux. This narrows the design's part 11 sentence on exit codes.
- **T32-R7 (the size before a discard).** A new request, `discard_preview`, returns the bytes and file count of the job's staging folder, counted on a worker thread; when the share does not answer it returns the reason and the question says the size is unknown.
- **T32-R8 (a developer command for sheets).** `lazymkv-dev sheet <job folder> [--text DIR] [--held] [--all]` builds and prints a sheet from a job folder. It serves the tests and the wording trial; it is not an owner command.
- **T32-R9 (`--overrides FILE`).** FILE is a `review.json`; the flag reads its `overrides` key.

## Review Focus

1. An answer that does not fit the job (a stream the title lacks, a path the sheet did not show, a word the item does not offer) or an `overrides` object with a misspelt key: refused naming it, nothing saved, the job unchanged. Tests in Tasks 2, 4 and 8.
2. A second review of one job that changes one track, and a restart between an approval and its Decide: the first review's other answers still apply. Tests in Tasks 2, 7 and 16.
3. A discard while the staging share is down: the job is `Discarded` at once, its data goes at a later pass, and nothing else is deleted. Test in Task 10.
4. A subtitle track with fewer than three events with text, an unread track, and a line with control characters or 500 characters: the sheet keeps one line per track and says why a track has none. Tests in Tasks 5 and 9.
5. Input that ends in the middle of the questions, a letter the item does not offer, and an engine that closes the connection after the sheet: nothing is sent in the first two, and the third exits 4. Tests in Task 14.

---

## Pull request 1: design

### Task 0: Start (controller)

**Files:**
- Create: `.superpowers/sdd/2026-11-24-phase-3-pr2-review/progress.md` (git-ignored ledger)
- Commit: `docs/superpowers/plans/2026-11-24-phase-3-pr2-review.md`

- [ ] **Step 1: Baseline.** On the branch that holds the design's commit, run `CARGO test --workspace --exclude lazymkv-ocr --locked --offline`. Expected: 0 failed. Write passed, failed and ignored to the ledger as N0.
- [ ] **Step 2: Commit this plan** as `docs: the plan for phase 3's pull requests 1 and 2 (design and review)`. Run the signature check of Global Constraints.

### Task 1: The spec's amendments

**Files:**
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md`
- Modify: `docs/superpowers/specs/2026-11-24-phase-3-design.md` (its status line only)

- [ ] **Step 1: Apply each row of the design's part 15** to the spec, in place, each changed sentence marked `(phase 3 design, <Yn or Zn>)` as the phase 2 amendments are marked. The rows: 4.5; 6.3's first paragraph and its two fallback sentences; D18; 7 "Retention"; 8.8 "Title confidence below..."; 10.1's table (the `Planned` row, a row for discard from a waiting state, a row for the collision at `Verified`); 10.4; 11; 14 (`direct_candidates_max`, default 4); 16 row 3; 17 row 3's fallback column. Add `Y1 to Y5` and `Z1 to Z28` to the paragraph after D23 that lists where rulings are recorded, and the date to the spec's first line.
- [ ] **Step 2: Do not touch** spec 17's rows 13 and 14: `retune` reads them (spec 15).
- [ ] **Step 3: Verify.** `CARGO test -p lazymkv-dev --locked --offline retune` passes, and `grep -c 'phase 3 design' docs/superpowers/specs/2026-10-04-lazymkv-design.md` prints 12 or more.
- [ ] **Step 4: Set the design's status line** to say the amendments are applied. Commit as `docs: the spec amended for phase 3 (Y1 to Y5, Z1 to Z28)`.
- [ ] **Step 5: Stop for prerequisite 1.** On the owner's word: push, open the pull request with no attribution line in its body, bind it with the app's pull request tools and read its CI.

---

## Pull request 2: review

Branch `phase-3-review` from `main` after pull request 1 has merged.

### Task 2: Merging and strict reading of answers

**Files:**
- Modify: `crates/core/src/remembered.rs`

**Interfaces:**
- Produces, in `lazymkv_core::remembered`:
  - `pub const OVERRIDE_KEYS: [&str; 6] = ["identity", "season", "titles", "audio", "subtitles", "collision"];`
  - `pub fn overrides_from_value(value: &serde_json::Value) -> Result<Overrides, String>`
  - `impl Overrides { pub fn merged(&self, later: &Overrides) -> Overrides; pub fn is_empty(&self) -> bool; }`

- [ ] **Step 1: Write the failing tests** in the file's `tests` module:

```rust
#[test]
fn an_unknown_key_in_the_answers_is_refused_naming_it() {
    let refused = overrides_from_value(&json!({"subtitle": []})).unwrap_err();
    assert_eq!(refused, "overrides: unknown key \"subtitle\" (identity, season, titles, audio, subtitles, collision)");
    assert_eq!(overrides_from_value(&json!([1])).unwrap_err(), "overrides: not an object");
    assert_eq!(overrides_from_value(&json!({})).unwrap(), Overrides::default());
    assert!(overrides_from_value(&json!({"audio": [{"title": 0, "stream": 1, "action": "louder"}]})).unwrap_err().starts_with("overrides: "));
}

#[test]
fn a_later_answer_replaces_the_earlier_one_for_the_same_track_and_keeps_the_rest() {
    let first = overrides_from_value(&json!({"subtitles": [
        {"title": 0, "stream": 9, "action": "keep", "role": "sdh"},
        {"title": 0, "stream": 33, "action": "drop"}]})).unwrap();
    let later = overrides_from_value(&json!({"subtitles": [{"title": 0, "stream": 9, "action": "keep", "role": "regular"}],
        "collision": [{"path": "M:/Movies/F (2000)/F (2000) - 2160p.mkv", "action": "keep_both"}]})).unwrap();
    let merged = first.merged(&later);
    assert_eq!(merged.subtitles_of(0).unwrap()[&9], SubtitleOverride::Keep { role: SubtitleRole::Regular });
    assert_eq!(merged.subtitles_of(0).unwrap()[&33], SubtitleOverride::Drop);
    assert_eq!(merged.collision_for("m:\\movies\\f (2000)\\f (2000) - 2160p.mkv"), Some(Collision::KeepBoth));
    assert!(Overrides::default().is_empty() && !merged.is_empty());
}
```

- [ ] **Step 2: Run** `CARGO test -p lazymkv-core --locked --offline remembered::tests`. Expected: FAIL, the functions are not defined.
- [ ] **Step 3: Implement.** `merged`: `identity` and `season` are `later`'s when `Some`; an entry of `later` replaces the entry with the same key (`titles` by title, `audio` and `subtitles` by title and stream, `collision` by `settings::same_path`), other entries are kept, and each list ends sorted by its key as the type's doc says.
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Commit** `core: merge review answers and read them strictly`.

### Task 3: The reducer's two new kinds of row

**Files:**
- Modify: `crates/core/src/job.rs`
- Test: `crates/core/src/job/tests.rs`

**Interfaces:**
- Produces: `Event::FileCollided { reasons: Vec<ReviewReason> }` (6.4 step 14: the rename met a file at the final path, or a final folder the share reports and the engine cannot open; `reasons` are the job's with `path_collision`).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn a_collision_at_filing_holds_the_job_at_the_gate_and_deletes_the_partial_file() {
    let reasons = vec![ReviewReason::SubtitleAmbiguous, ReviewReason::PathCollision];
    let (state, actions) = reduce(&State::Verified, &Event::FileCollided { reasons: reasons.clone() }).unwrap();
    assert_eq!(state, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(actions, vec![Action::DeletePartialFile, Action::Notify { event: NotifyEvent::NeedsReview { reasons } }]);
    assert!(reduce(&State::Remuxing, &Event::FileCollided { reasons: vec![] }).is_err());
}

#[test]
fn a_waiting_job_can_be_discarded() {
    let back = || Box::new(State::Ripping);
    for waiting in [
        State::AwaitingShare { return_to: back() }, State::AwaitingSpace { return_to: back() },
        State::AwaitingMakeMkv { return_to: back() }, State::AwaitingNetwork { return_to: back() },
        State::AwaitingLookup { return_to: Box::new(State::Inspecting) }, State::AwaitingTool { return_to: back() },
    ] {
        assert_eq!(reduce(&waiting, &Event::ReviewDiscarded).unwrap(), (State::Discarded, vec![Action::DeleteJobData]));
    }
    for working in [State::Ripping, State::Remuxing, State::Filed, State::Discarded] {
        assert!(reduce(&working, &Event::ReviewDiscarded).is_err());
    }
}
```

- [ ] **Step 2: Run** `CARGO test -p lazymkv-core --locked --offline job::tests`. Expected: FAIL to compile, `FileCollided` is not a variant.
- [ ] **Step 3: Implement** both rows in `review_line`, and add `FileCollided` to every list of events the tests enumerate, so `no_state_and_event_pair_is_held_by_two_rules` covers it.
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Commit** `core: hold a collision found at filing, and let a waiting job be discarded`.

### Task 4: The review document

**Files:**
- Create: `crates/core/src/review_doc.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod review_doc;`)

**Interfaces:**
- Consumes: `record::JobRecord`, `disc::Disc`, `decision::FileDecision`, `remembered::{Overrides, AudioOverride, SubtitleOverride, Collision}`, `review::{ReviewReason, Noted}`.
- Produces, in `lazymkv_core::review_doc` (all `Clone, Debug, PartialEq, Serialize, Deserialize`, enums `snake_case`):

```rust
pub const DOC_FORMAT: &str = "lazymkv review document v1";
pub enum HeldAs { Gate, AfterCopy, Filed }
pub enum PointKind { Identity, Titles, Audio, Subtitles, Collision, Rip }
pub enum ItemKind { Audio, Subtitle, Collision }
pub struct ReviewDoc { pub format: String, pub job: String, pub held: HeldAs, pub since_unix: u64,
    pub identity: DocIdentity, pub reasons: Vec<ReviewReason>, pub notes: Vec<Noted>,
    pub points: Vec<Point>, pub titles: Vec<DocTitle> }
pub struct DocIdentity { pub step: String, pub confidence: f64, pub title: Option<String>, pub year: Option<i32> }
pub struct Point { pub kind: PointKind, pub reasons: Vec<ReviewReason>, pub title_key: Option<String>,
    pub sentence: String, pub answerable: bool, pub items: Vec<String> }
pub struct DocTitle { pub key: String, pub scan_title: u32, pub seconds: Option<u32>, pub chapters: Option<u32>,
    pub bytes: Option<u64>, pub staged_path: Option<String>, pub final_path: Option<String>, pub items: Vec<Item> }
pub struct Item { pub id: String, pub kind: ItemKind, pub title: u32, pub stream: Option<u32>, pub path: Option<String>,
    pub describe: String, pub stands: String, pub order_if_kept: Option<u32>, pub answers: Vec<String>,
    pub evidence: Vec<Evidence> }
pub struct Evidence { pub kind: String, pub text: String }

pub struct DocInputs<'a> {
    pub record: &'a JobRecord, pub disc: &'a Disc,
    pub staged: &'a BTreeMap<String, String>,              // title key -> the staged file's path as shown
    pub lines: &'a BTreeMap<(String, u32), Vec<String>>,   // (title key, stream) -> subtitle lines
    pub unread: &'a BTreeMap<(String, u32), String>,       // (title key, stream) -> why it has no lines
    pub collisions: &'a BTreeMap<String, String>,          // colliding path -> the file there, "<bytes> bytes, <date>"
    pub all: bool,
}
pub fn build(inputs: &DocInputs<'_>) -> Result<ReviewDoc, String>;

pub enum LabelKind { Audio, Subtitle }
pub struct ReviewLabel { pub title: u32, pub stream: u32, pub kind: LabelKind, pub label: String }
pub struct Answers { pub overrides: Overrides, pub labels: Vec<ReviewLabel> }
/// The answers that `words` give: one (item id, answer word) pair per item changed.
pub fn answers_of(doc: &ReviewDoc, words: &[(String, String)]) -> Result<Answers, String>;
/// Ok when every entry of `overrides` names an item of `doc` and an answer that item offers.
pub fn check(doc: &ReviewDoc, overrides: &Overrides) -> Result<(), String>;
```

Rules the tests pin, from the design's part 6.2 and rulings T32-R1 to T32-R4:
- `build` is Err `job <id> is <state>: there is nothing to review` unless the job is held at the gate or filed; `held` is `Gate` or `Filed`, and `since_unix` the record's `updated_unix` or `filed_unix`.
- Points come in spec 8.8's order of reasons. A held job has a point only for reasons it carries; with `all`, or for a filed job, every title also gets an audio and a subtitles point listing all its items. Identity and titles points have `answerable` false and no items (T32-R1); a rip point has no items (T32-R4).
- An item's `stands` is the word of what the decision did; a collision item's is empty. `order_if_kept` is the track's place among the kept audio tracks, or the count of kept tracks for a dropped one.
- An item's `describe` is a language word, two spaces, then what the track is. The language word is `English` for an English tag (`lang::is_english`), `no language` for none, else the tag as the scan wrote it. A subtitle track then has `<events, with thousands separators> lines` (nothing when the count is unknown); an audio track has its 8.10 name when kept (`TrueHD Atmos 7.1`), else its codec and channel count from the scan. A collision's `describe` is its path.
- Evidence kinds: `lines` (one per subtitle line), `unread`, `events`, `note` (each note naming the stream), `verdict`, `file` (a collision's existing file).
- `answers_of`: `default` gives `AudioOverride::Keep { order, default: true }`, `keep` the same with `default: false`, `commentary` `AudioOverride::Commentary` and a label `commentary`; a subtitle role gives `SubtitleOverride::Keep { role }` and a label of that word, a subtitle `commentary` gives `Drop` and a label `commentary`, `drop` gives `Drop` and no label; a word equal to the item's `stands` gives nothing. Errors: `no item "<id>" on this sheet`, `"<word>" is not an answer for <id> (<the item's words>)`.

- [ ] **Step 1: Write the failing tests** over a fixture record with one title, kept audio streams 1 (default) and 8 (noted `possible_commentary`), kept subtitle streams 9 and 33 (both `regular`, reason `subtitle_ambiguous`), dropped subtitle streams 10 (`derived_forced_only_empty`) and 11 (`other_language`):
  - `a_held_job_has_one_point_per_reason_in_the_table_order`: `points` are one `Subtitles` point with `items == ["t00-00801 sub 9", "t00-00801 sub 33"]`, `answerable` true, and the sentence `Subtitles: two full English tracks that do not match each other. lazymkv cannot tell which one carries the film's dialogue.`
  - `tracks_dropped_for_their_language_or_as_derived_are_never_items`: no item's id ends in `sub 10` or `sub 11`.
  - `all_adds_an_audio_point_with_every_item`: with `all`, an `Audio` point lists `audio 1` and `audio 8`; item 1 has `stands == "default"`, item 8 `stands == "keep"` and an evidence entry of kind `note`.
  - `a_collision_is_one_item_with_no_standing_answer`: with reason `path_collision` and one entry in `collisions`, the item has `answers == ["keep_both", "replace"]`, `stands == ""` and evidence kind `file`.
  - `an_identity_reason_gives_a_point_that_cannot_be_answered_yet`.
  - `a_rip_reason_gives_a_point_with_no_item`: `read_errors` gives a `Rip` point, `answerable` false, whose sentence says that filing accepts the rip as it is and that the other way is to discard and rip again.
  - `a_missing_original_language_heads_the_audio_point`: `original_language_missing` gives one `Audio` point that lists the kept audio items.
  - `a_job_that_is_ripping_has_nothing_to_review`: the Err text above with `Ripping`.
  - `answers_become_overrides_and_labels`: `[("t00-00801 sub 9", "sdh"), ("t00-00801 sub 33", "commentary")]` gives `Keep { role: Sdh }` for 9, `Drop` for 33, labels `sdh` and `commentary`; `("t00-00801 sub 9", "regular")` alone gives empty answers.
  - `an_answer_for_a_track_the_sheet_lacks_is_refused`: both Err texts above; `check` refuses an override for stream 11 and one for a path not shown.
  - `the_document_round_trips_through_json`.
- [ ] **Step 2: Run** `CARGO test -p lazymkv-core --locked --offline review_doc`. Expected: FAIL, the module does not exist.
- [ ] **Step 3: Implement** `build`, `answers_of` and `check`. Each reason's sentence is one constant per `ReviewReason`, written as plain words with no rule names or code words.
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Commit** `core: the review document and its answers`.

### Task 5: Subtitle lines and the sheet

**Files:**
- Modify: `crates/core/src/review_doc.rs`

**Interfaces:**
- Produces:
  - `pub const LINE_MOMENTS: [f64; 3] = [0.25, 0.50, 0.75];` and `pub const LINE_WIDTH: usize = 60;`
  - `pub fn sample_lines(events: &[(f64, Vec<String>)], seconds: f64) -> Vec<String>` (T32-R5: `events` are (start in seconds, the event's text lines) in time order)
  - `pub fn render(doc: &ReviewDoc) -> String`: the sheet of the design's part 6.3 without the questions.
  - `pub fn question(item: &Item) -> String`: one item's question line, `track 9 is:  r regular   s SDH   f forced   c commentary (dropped)   d drop     [Enter = r]`.
  - `pub fn letter_word(item: &Item, letter: &str) -> Option<&'static str>`: the word of a letter for that item's kind (T32-R3).

- [ ] **Step 1: Write the failing tests:**
  - `lines_are_taken_at_the_three_moments`: events at 100 s, 1500 s, 3100 s and 4600 s of a 6000 s title give the lines of 1500, 3100 and 4600.
  - `a_track_with_one_event_gives_one_line`, and `no_event_with_text_gives_none`.
  - `a_line_is_cut_and_cleaned`: an event whose text is 500 characters with a tab and an escape character gives one line of 60 characters with neither; two text lines of one event are joined by a space.
  - `the_sheet_for_two_subtitle_tracks_reads_as_the_design_shows_it`: `render` of Task 4's fixture, with two lines per track and a staged path, equals a literal expected string in the test, whose point block is:

```
1. Subtitles: two full English tracks that do not match each other.
   lazymkv cannot tell which one carries the film's dialogue.

   track 9   English  1,224 lines   "Where are you going?" / "[door slams]"
   track 33  English  1,410 lines   "Hi, I'm the director." / "We shot this scene in March."
   To watch first: S:\staging\job\rip\t00-00801\title.mkv

   As it stands: track 9 regular, track 33 regular.
```

  - `an_unread_track_says_why_in_place_of_lines`: the track's row ends `(not read: <reason>)`.
  - `a_point_that_cannot_be_answered_says_when_it_can`: an identity point's block ends `This can be answered from a later version of lazymkv.`
  - `the_question_line_marks_what_enter_keeps`, and `a_letter_the_item_does_not_offer_is_none`.
- [ ] **Step 2: Run** `CARGO test -p lazymkv-core --locked --offline review_doc`. Expected: FAIL.
- [ ] **Step 3: Implement.** A point's `As it stands:` line lists its items in order as `track <stream> <stands>`, or `<path>: not answered` for a collision. The sheet's head line is `Held since <day month>.` or `Filed on <day month>.`, then `Identified by <step in words> (<confidence>).`, then `<n> point(s) need(s) your answer.` or `Nothing is open.`; the foot is one `Files as: <final path>` line per title. Dates are formatted from `since_unix` in UTC.
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Commit** `core: subtitle lines and the review sheet`.

### Task 6: The new requests

**Files:**
- Modify: `crates/core/src/ipc.rs`

**Interfaces:**
- Produces, in `Request`: `Review { job: String, all: bool }`; `DiscardPreview { job: String }`; and `ReviewAction` gains `labels: Option<Value>`. Wire types `review` (`all` defaults to false), `discard_preview`, and `review_action`'s `labels` key.

- [ ] **Step 1: Write the failing tests** by adding rows to the table of the existing request-parsing test:

```rust
(r#"{"v":1,"type":"review","id":2,"payload":{"job":"a-1"}}"#, Request::Review { job: job(), all: false }),
(r#"{"v":1,"type":"review","id":2,"payload":{"job":"a-1","all":true}}"#, Request::Review { job: job(), all: true }),
(r#"{"v":1,"type":"discard_preview","id":2,"payload":{"job":"a-1"}}"#, Request::DiscardPreview { job: job() }),
(r#"{"v":1,"type":"review_action","id":2,"payload":{"job":"a-1","action":"approve","labels":[]}}"#,
    Request::ReviewAction { job: job(), action: "approve".to_string(), overrides: None, labels: Some(json!([])) }),
```

- [ ] **Step 2: Run** `CARGO test -p lazymkv-core --locked --offline ipc`. Expected: FAIL to compile.
- [ ] **Step 3: Implement**, and add `labels: None` to the two existing `ReviewAction` rows of that table. The engine's and `lazymkvctl`'s uses of `ReviewAction` gain the field with no behaviour change (`labels` ignored until Task 8).
- [ ] **Step 4: Run** the whole run of Global Constraints. Expected: PASS.
- [ ] **Step 5: Commit** `core: the review and discard_preview requests`.

### Task 7: review.json, and Decide with the answers

**Files:**
- Create: `crates/engine/src/review_store.rs`
- Modify: `crates/engine/src/lib.rs` (the module), `crates/engine/src/file_phase.rs` (`replay_options`, `engine_decide_inputs`), `crates/engine/src/relay.rs` (`child_words`)
- Modify: `crates/core/src/review_doc.rs` (the two constants), `crates/dev/src/cmd/plan.rs` (`PlanOptions`, `FLAGS`), `crates/dev/src/cmd/decide.rs` (its flags, `read_decide_inputs`), `crates/dev/src/lib.rs` (the two synopses), `crates/ctl/src/lib.rs` (`DECIDE_PATH_FLAGS` gains `--overrides`, so a relative path is relayed made absolute)

**Interfaces:**
- Produces, in `lazymkv_core::review_doc` (so that `dev` and the engine name one file and one format): `pub const REVIEW_FILE: &str = "review.json";` and `pub const REVIEW_FORMAT: &str = "lazymkv review v1";`
- Produces, in `crate::review_store`:
  - `pub(crate) struct Reviewed { pub time_unix: u64, pub action: String, pub reasons: Vec<ReviewReason> }`
  - `pub(crate) struct ReviewFile { pub format: String, pub overrides: Overrides, pub reviews: Vec<Reviewed> }`
  - `pub(crate) fn read(job_dir: &Path) -> Result<Option<ReviewFile>, String>` (a missing file is `Ok(None)`; a file that does not parse or has another format is Err naming it)
  - `pub(crate) fn record(job_dir: &Path, answers: &Overrides, review: Reviewed) -> Result<ReviewFile, String>` (reads, merges with `Overrides::merged`, appends, writes with `lazymkv_platform::write_atomic`)
- Produces, in `lazymkv_dev::cmd::plan`: `PlanOptions.overrides: Option<PathBuf>`, and `pub fn replay_overrides(ctx: &Ctx, options: &PlanOptions) -> Result<Option<Overrides>, Refused>` (T32-R9).

- [ ] **Step 1: Write the failing tests.**
  - In `review_store.rs`: `a_second_review_adds_to_the_first` (two `record` calls: the file holds the merged overrides and two `reviews`); `a_file_of_another_format_is_refused_naming_it`; `a_missing_file_is_no_review`.
  - In `crates/dev/src/cmd/decide.rs`'s tests, beside the `--remembered` test: `overrides_from_a_review_file_reach_the_decision`: with a `review.json` that drops one of the kept subtitle streams of the tests' recording `disc-b`, `decide ... --overrides review.json` keeps one subtitle track fewer than without it, and that track's drop reason is `reviewed`; `--overrides missing.json` exits 2 naming the file.
  - In `crates/engine/src/relay.rs`'s tests: `a_job_with_a_review_file_gets_overrides_in_the_childs_words`: the child's words hold `--overrides <jobs>/<id>/review.json` only when the file exists.
- [ ] **Step 2: Run** `CARGO test -p lazymkv-engine -p lazymkv-dev --locked --offline review`. Expected: FAIL.
- [ ] **Step 3: Implement.** `replay_options` gives `overrides` when `review.json` is a file. `engine_decide_inputs` and `read_decide_inputs` set `DecideInputs.overrides` from `replay_overrides`; `plan` reads the flag and passes it as `PlanInputs.overrides`. `--overrides` joins both commands' flag lists and synopses.
- [ ] **Step 4: Run** the whole run. Expected: PASS, the goldens unchanged.
- [ ] **Step 5: Commit** `engine: keep review answers in the job folder and decide with them`.

### Task 8: Approval with answers

**Files:**
- Modify: `crates/engine/src/owner.rs` (`review_action`, `on_request`), `crates/engine/src/ipc_server.rs` (the refusal texts), `crates/engine/src/review_store.rs` (the labels file)
- Test: `crates/fake-tools/tests/engine_ipc.rs`

**Interfaces:**
- Consumes: `review_doc::{build, check, DocInputs, ReviewLabel}`, `remembered::overrides_from_value`, `review_store::record`.
- Produces: `review-labels.json` in the job folder, `{"format": "lazymkv review labels v1", "labels": [{"time_unix", "title_key", "stream", "kind", "label", "verdict"}]}`, where `verdict` is the item's `stands` on the sheet the answer was checked against; entries are appended, never rewritten. `pub(crate) const LABELS_FILE: &str = "review-labels.json";` in `review_store`.

- [ ] **Step 1: Write the failing tests** in `engine_ipc.rs`, each on the `movie-direct-two-subs` scenario held in `NeedsReview` (the pattern of `an_approved_job_decides_again_from_its_kept_inputs_and_files`). `T` is the scenario's scan title and `S` one of its two kept subtitle streams, both read from the held job's record in the test:
  - `approve_with_a_subtitle_answer_files_the_job_with_that_answer`: `review_action` with `overrides` `{"subtitles": [{"title": T, "stream": S, "action": "drop"}]}` and one label replies `ok`; the job reaches `Filed`; its `decide.json` keeps one subtitle track fewer and names the drop `reviewed`; `review.json` holds the override and one review with the reasons shown; `review-labels.json` holds the label.
  - `an_answer_for_a_stream_the_job_lacks_is_refused_and_nothing_is_saved`: stream 99 gives the error `no item "<key> sub 99" on this sheet`; no `review.json`; the state is unchanged.
  - `a_misspelt_key_is_refused_naming_it`: `{"subtitle": []}` gives Task 2's text.
  - `identity_and_titles_answers_wait_for_a_later_version`: `{"season": 2}` gives `this version does not take season answers yet` (replacing `approve_with_overrides_is_refused_naming_phase_3`).
  - `an_unknown_action_is_refused`: `set_identity` gives `"set_identity" is not a review action (approve)`.
- [ ] **Step 2: Run** `CARGO test -p fake-tools --locked --offline --test engine_ipc approve`. Expected: FAIL on the new tests.
- [ ] **Step 3: Implement** `review_action` for `approve`: read the overrides strictly; refuse T32-R1's keys; build the document from the record and the job folder's `scan.txt` with empty `staged`, `lines`, `unread` and `collisions` (local reads only) and `check` the overrides against it; then `review_store::record`, append the labels, and queue `ReviewApproved` with the merged overrides and the summary's reasons for the file-phase slot, as today. `approve` with no answers behaves as before and writes no file.
- [ ] **Step 4: Run** the whole run. Expected: PASS.
- [ ] **Step 5: Commit** `engine: approve a held job with track and collision answers`.

### Task 9: The review request

**Files:**
- Create: `crates/engine/src/review_build.rs`
- Create: `crates/dev/src/cmd/sheet.rs` (the reader of subtitle lines only; Task 13 adds the command to it), with `pub mod sheet;` in `crates/dev/src/cmd/mod.rs`
- Modify: `crates/engine/src/lib.rs`, `crates/engine/src/owner.rs` (`Msg`, `on_request`, a handler for the worker's answer)
- Test: `crates/fake-tools/tests/engine_ipc.rs`

**Interfaces:**
- Consumes: `review_doc::{build, sample_lines, DocInputs}`; `file_phase::staged_text`; `staged::tuning_dir`; `tuning::DATASET`; `lazymkv_core::recording::files::read_jsonl`; `lazymkv_core::verify::{subs_streams, by_scan_stream}`, the mapping `lazymkv_dev::cmd::decide` uses for the text's decisions.
- Produces:
  - in `lazymkv_dev::cmd::sheet`: `pub fn subtitle_lines(rec: &Recording, label: &str, text: &Path, seconds: f64) -> Result<BTreeMap<u32, Vec<String>>, String>`: scan stream to its `sample_lines`, for every `sub-<n>.ocr.jsonl` of `text`, `n` mapped as the decision's text is; Err names a file that could not be read. Its own test: two streams of a small recording give their lines under their scan stream numbers.
  - `pub(crate) fn spawn_review(job: JobContext, all: bool, shares: Arc<dyn ShareConnector>, reply: impl FnOnce(Result<ReviewDoc, String>) + Send + 'static) -> JoinHandle<()>`
  - `Msg::ReviewBuilt { client: u64, id: Option<u64>, answer: Result<Value, String> }`

- [ ] **Step 1: Write the failing tests:**
  - `review_returns_the_document_of_a_held_job_with_its_subtitle_lines`: on `movie-direct-two-subs` held, `review` replies with a document of format `lazymkv review document v1`, one `subtitles` point with two items, each with evidence of kind `lines` whose texts are the scenario's stand-in OCR lines, and a `staged_path` under the staging root.
  - `review_of_a_job_whose_staging_share_is_down_still_answers`: with the fake share unreachable, the document comes without lines and each subtitle item has evidence of kind `unread` whose text names the share.
  - `review_of_a_job_with_nothing_to_review_is_an_error_naming_its_state`.
  - `no_subtitle_line_reaches_a_log_or_a_push`: after a `review`, neither `job.log`, `engine.log` nor any push line read by a second client holds any of the lines.
- [ ] **Step 2: Run** `CARGO test -p fake-tools --locked --offline --test engine_ipc review_`. Expected: FAIL.
- [ ] **Step 3: Implement.** The owner thread only starts the worker and later sends its answer. The worker reads `scan.txt` and each title's `subs/index.json` and `mkvmerge.json` from the job folder, the OCR text from `staged_text`, else from `<tuning_root>\dataset\<job id>\<key>\subs-ocr`, and, for a job with `path_collision`, the size and modification day of each path in `record.collisions`. A root that does not answer gives `unread` evidence, never an error.
- [ ] **Step 4: Run** the whole run. Expected: PASS.
- [ ] **Step 5: Commit** `engine: answer review with the job's document`.

### Task 10: Discard

**Files:**
- Modify: `crates/engine/src/owner.rs` (`on_request`, `perform`, `start_cleanup`), `crates/engine/src/cleanup.rs` (`CleanupJob`, `run_pass`), `crates/engine/src/ipc_server.rs` (texts), `crates/core/src/record.rs` (`TuningCopy::Dropped`)
- Test: `crates/engine/src/cleanup.rs` (its `tests`), `crates/fake-tools/tests/engine_ipc.rs`

**Interfaces:**
- Produces: `CleanupJob.library_roots: Vec<String>`; `pub(crate) fn discard_due(job: &CleanupJob) -> bool` (the record is `Discarded`); `TuningCopy::Dropped`; replies of `discard_preview`: `{"bytes": n, "files": n}` or `{"unknown": "<reason>"}`.

- [ ] **Step 1: Write the failing tests.**
  - In `cleanup.rs`: `a_discarded_job_has_its_staging_folder_and_partial_files_deleted_at_any_age`; `a_discarded_job_under_an_unreached_root_keeps_its_data_and_is_logged`; `a_partial_file_of_another_job_is_left_alone` (only `<job id>-*.mkv.partial` files of `.lazymkv-partial` go, and a link is never followed).
  - In `engine_ipc.rs`: `a_held_job_is_discarded_and_its_data_goes` (state `Discarded`, the staging folder gone after the pass, the job folder kept, history lists it, the library untouched); `a_failed_job_and_a_waiting_job_can_be_discarded`; `discard_of_a_running_job_is_refused_naming_cancel` (`job <id> has a step running: cancel it first`); `discard_of_a_filed_job_is_refused_naming_its_state`; `a_discard_while_the_share_is_down_deletes_at_a_later_pass` (Review Focus 3: `Discarded` at once, data still there, then gone after the share returns and a pass runs); `a_discarded_jobs_pending_tuning_copy_is_dropped`; `discard_preview_counts_the_staging_folder`.
- [ ] **Step 2: Run** `CARGO test -p lazymkv-engine --locked --offline cleanup` and `CARGO test -p fake-tools --locked --offline --test engine_ipc discard`. Expected: FAIL.
- [ ] **Step 3: Implement.** `discard` feeds `ReviewDiscarded` when no step of the job runs and the reducer takes it. `perform` answers `DeleteJobData` on the owner thread: the job leaves the token queue, the slot queue and the pending slot events, its promises are released, a pending tuning copy becomes `Dropped`, and a cleanup pass starts at once. `run_pass` deletes a discarded job's staging folder and partial files whenever its root answers. `discard_preview` walks the staging folder on a worker thread.
- [ ] **Step 4: Run** the whole run. Expected: PASS.
- [ ] **Step 5: Commit** `engine: discard a held, failed or waiting job`.

### Task 11: Collisions at filing, and "replace"

**Files:**
- Modify: `crates/engine/src/steps.rs` (`file`), `crates/engine/src/file_phase.rs`, `crates/engine/src/ipc_server.rs` (`cancel_event`, `failure_event` stay as they are)
- Test: `crates/fake-tools/tests/engine_file_phase.rs`

**Interfaces:**
- Consumes: `Event::FileCollided` (Task 3); `Overrides::collision_for`.
- Produces: `pub(crate) const REPLACED_FOLDER: &str = "replaced";` in `file_phase`, the folder `<staging>\<job id>\replaced\`.

- [ ] **Step 1: Write the failing tests:**
  - `a_file_that_appears_before_the_rename_holds_the_job_with_path_collision`: a file written at the final path while the job is in `Remuxing` gives `NeedsReview`, a `needs_review` notification with `path_collision`, the path in `record.collisions`, and no partial file left (replaces the test that expects `Failed` there).
  - `a_final_folder_that_cannot_be_opened_holds_the_job_too` (L7, on the fake share's present-but-closed folder).
  - `keep_both_files_under_the_copy_label`: approval with `{"collision": [{"path": P, "action": "keep_both"}]}` files `... - 2160p 2.mkv` and leaves the existing file.
  - `replace_moves_the_existing_file_into_the_jobs_staging_folder_and_files`: after approval with `replace`, the final path holds the job's file and `<staging>/<job>/replaced/` holds the old one, byte for byte.
  - `replace_with_nothing_at_the_path_any_more_just_files`: the existing file is deleted after the approval; the job files and `replaced/` is not made.
  - `a_replaced_file_that_cannot_be_moved_fails_the_step_and_leaves_both_files`: the move's error gives `FileFailed`, class `Unknown`, naming both paths; the library file and the partial file are untouched.
- [ ] **Step 2: Run** `CARGO test -p fake-tools --locked --offline --test engine_file_phase collision`. Expected: FAIL.
- [ ] **Step 3: Implement.** `file` feeds `FileCollided`, with the job's reasons and `path_collision`, where it fed `FileFailed` for an existing final path and for L7; it adds `path_collision` to that title's decision and the path to `record.collisions` in the same posted edit. For a path answered `replace`, `file` moves the file found there into `REPLACED_FOLDER` under its own name (a name already there gets ` 2`, ` 3` and so on before the extension), then renames as before. The file moved aside is not checked against what the sheet showed: it is kept, so nothing is lost.
- [ ] **Step 4: Run** the whole run. Expected: PASS.
- [ ] **Step 5: Commit** `engine: hold a collision found at filing, and replace on the owner's answer`.

### Task 12: Remember after filing

**Files:**
- Modify: `crates/engine/src/store.rs`, `crates/engine/src/steps.rs` (`file`)
- Test: `crates/fake-tools/tests/engine_file_phase.rs`

**Interfaces:**
- Consumes: `decision::remember(fingerprint, disc, plan, decisions, overrides) -> Result<Remembered, String>`.
- Produces: `Store::write_remembered(&self, remembered: &Remembered) -> io::Result<()>` (atomic, at the path `Store::remembered` reads).

- [ ] **Step 1: Write the failing tests:**
  - `a_reviewed_job_is_remembered_when_it_files`: after Task 8's approval and filing, `<data>/remembered/<fingerprint>.json` parses as `Remembered` and holds the dropped subtitle stream's signature with `drop`.
  - `the_same_disc_ripped_again_files_without_a_hold`: the first job's filed file is removed from the library (its path would collide), then `rip_again`, a second insertion, and the new job reaches `Filed` with no `needs_review` notification.
  - `a_job_approved_unchanged_writes_no_remembered_file`.
  - `a_remembered_file_that_cannot_be_written_is_logged_and_the_job_stays_filed`.
- [ ] **Step 2: Run** `CARGO test -p fake-tools --locked --offline --test engine_file_phase remember`. Expected: FAIL.
- [ ] **Step 3: Implement** at the end of `file`, after the last title's record is posted: when the job's `review.json` holds answers, call `remember` and write the result; an Err of either goes to `job.log` as `remember: <error>` and feeds nothing.
- [ ] **Step 4: Run** the whole run. Expected: PASS.
- [ ] **Step 5: Commit** `engine: remember a reviewed job's answers when it files`.

### Task 13: lazymkv-dev sheet

**Files:**
- Modify: `crates/dev/src/cmd/sheet.rs` (Task 9 made it, with `subtitle_lines`)
- Modify: `crates/dev/src/lib.rs` (`COMMANDS`: `("sheet", cmd::sheet::run as Command, "<job folder> [--text DIR] [--held] [--all]")`)

**Interfaces:**
- Consumes: `review_doc::{build, render, DocInputs}`; `subtitle_lines` (Task 9).
- Produces: `pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32`, the command of T32-R8. Exit 0 with the sheet on standard output; 2 for an argument error, a folder with no `state.json`, or a job with nothing to review, naming it.

- [ ] **Step 1: Write the failing tests** in the new file, over a job folder written by the test: `a_held_job_folder_prints_its_sheet` (equals `render` of the built document); `held_shows_a_filed_job_as_it_was_held` (`--held` turns a `Filed` record with a `subtitle_ambiguous` decision into the one-point sheet); `text_adds_the_lines` (with `--text DIR` holding `sub-0.ocr.jsonl` and `sub-1.ocr.jsonl`, each track's row carries its lines); `a_folder_without_a_state_file_exits_2_naming_it`.
- [ ] **Step 2: Run** `CARGO test -p lazymkv-dev --locked --offline sheet`. Expected: FAIL.
- [ ] **Step 3: Implement.** `--held` sets the record's state to `NeedsReview { held: DispositionGate }` in memory before `build`. The command writes nothing.
- [ ] **Step 4: Run** the whole run. Expected: PASS (any test that pins the command table gains `sheet`).
- [ ] **Step 5: Commit** `dev: print a job folder's review sheet`.

### Task 14: lazymkvctl review

**Files:**
- Create: `crates/ctl/src/review_cmd.rs`
- Modify: `crates/ctl/src/lib.rs` (`Env.interactive: bool`; dispatch of `review`), `crates/ctl/src/main.rs` (`interactive` from `std::io::IsTerminal` on standard input), `crates/ctl/src/engine_cmds.rs` (`ENGINE_COMMANDS` gains `("review", "review JOB [--all] [--json] [--answer ID=WORD ...] [--yes]")`)
- Modify: every other `Env { .. }` literal, which gains `interactive: false`: `crates/ctl/src/tests.rs`, `crates/ctl/src/engine_cmds.rs`'s tests, and `crates/fake-tools/tests/ctl_engine.rs`, `engine_native.rs` and `engine_command.rs`

**Interfaces:**
- Consumes: `review_doc::{ReviewDoc, render, question, letter_word, answers_of}`; the engine requests `review`, `review_action`, `discard_preview` and `discard`.
- Produces: `pub(crate) fn run(argv: &[String], env: Env) -> i32`. One connection: request 1 is `review`, request 2 `review_action` or `discard`. Exit 0 when the engine accepted, when the owner said "not yet", or when only the sheet was printed; 1 when the engine refused; 2 for an argument error; 4 when no engine answered or it closed the connection.

- [ ] **Step 1: Write the failing tests** with the file's own copy of `engine_cmds`' `call` and a peer that answers two requests, over a fixture document of two subtitle tracks:
  - `the_sheet_is_printed_and_nothing_is_sent_without_a_terminal_or_answers`.
  - `guided_answers_send_one_review_action`: standard input `s\nc\ny\n` with `interactive` true sends `review_action` whose `overrides.subtitles` are `Keep sdh` for the first track and `Drop` for the second and whose `labels` are `sdh` and `commentary`; the output ends with the one-line form `lazymkvctl review <job> --answer "<id>=sdh" --answer "<id>=commentary" --yes` and the sentence `Accepted. The job is filing; watch it with: lazymkvctl watch`.
  - `an_audio_point_takes_keep_default_commentary_and_drop`: on a fixture with an open audio point of two tracks, `p\nc\ny\n` sends `Keep { default: true }` for the first and `Commentary` for the second, with one label `commentary`.
  - `enter_keeps_every_item_and_yes_approves_unchanged`: `\n\ny\n` sends `review_action` with no overrides.
  - `not_yet_sends_nothing`: `s\nc\nn\n` sends nothing after `review`, prints `Nothing was changed.` and exits 0.
  - `a_letter_the_item_does_not_offer_is_asked_again` (`q\ns\n...`: the question line is printed twice).
  - `input_that_ends_in_the_middle_sends_nothing` (Review Focus 5: `s\n` then end of input; exit 0, `Nothing was changed.`).
  - `answers_on_the_command_line_need_yes`: `--answer` without `--yes` prints what would be sent and sends nothing; with `--yes` it sends the same request as the guided answers.
  - `an_answer_word_the_item_lacks_exits_2_before_anything_is_sent`.
  - `x_asks_for_the_word_discard`: `x\ndiscard\n` sends `discard_preview` then `discard`; `x\nno\n` sends neither.
  - `replace_asks_for_the_word_replace`, on a collision fixture.
  - `json_prints_the_document_itself`.
  - `subtitle_lines_go_to_standard_output_only`: after a guided review that the engine refuses, standard error holds the engine's words and none of the fixture's lines.
  - `a_refused_answer_exits_1_with_the_engines_words`, and `a_connection_closed_after_the_sheet_exits_4`.
- [ ] **Step 2: Run** `CARGO test -p lazymkv-ctl --locked --offline review_cmd`. Expected: FAIL.
- [ ] **Step 3: Implement.** Questions are asked point by point in the sheet's order, one `question` line per item, then `File it now?  y yes   n not yet   x discard the job`. A point that is not answerable is shown and not asked. Pushes that arrive between the two requests are skipped.
- [ ] **Step 4: Run** the whole run. Expected: PASS.
- [ ] **Step 5: Commit** `ctl: review a job from the terminal`.

### Task 15: lazymkvctl discard

**Files:**
- Modify: `crates/ctl/src/review_cmd.rs` (the shared confirmation), `crates/ctl/src/engine_cmds.rs` (`ENGINE_COMMANDS` gains `("discard", "discard JOB [--yes]")`), `crates/ctl/src/lib.rs`

**Interfaces:**
- Produces: `pub(crate) fn discard(argv: &[String], env: Env) -> i32`, and `fn confirm_discard(job: &str, preview: &Value, input: &mut dyn BufRead, out: &mut dyn Write) -> bool`, which Task 14's `x` uses.

- [ ] **Step 1: Write the failing tests:** `discard_asks_with_the_size_and_sends_on_the_word` (the question is `Discard deletes this job's rip: <n> files, <size>. The disc would have to be ripped again. Type discard to go on:`); `any_other_word_sends_nothing_and_exits_0`; `yes_skips_the_question`; `an_unknown_size_says_so` (`... its size is unknown (<reason>) ...`); `without_a_terminal_and_without_yes_it_exits_2_saying_so`.
- [ ] **Step 2: Run** `CARGO test -p lazymkv-ctl --locked --offline discard`. Expected: FAIL.
- [ ] **Step 3: Implement.** Sizes print in GB with one decimal.
- [ ] **Step 4: Run** the whole run. Expected: PASS.
- [ ] **Step 5: Commit** `ctl: discard a job`.

### Task 16: Whole jobs

**Files:**
- Create: `crates/fake-tools/tests/engine_review.rs`
- Modify: `crates/fake-tools/tests/ctl_engine.rs`, `crates/fake-tools/tests/compare_job.rs`

**Interfaces:**
- Consumes: the harness and `Client` of `crates/fake-tools/tests/common`.

- [ ] **Step 1: Write the tests** (they should pass at once; a failure is a fault in Tasks 7 to 12):
  - `a_job_held_with_two_english_subtitle_tracks_is_reviewed_and_filed_with_one`: the design's ending for this pull request, from insertion to `Filed`, through `review` and `review_action`; the filed file's stand-in `mkvmerge` arguments keep one subtitle track.
  - `a_restart_between_the_approval_and_the_decision_keeps_the_answers` (Review Focus 2): the engine restarts in `Decided`; the job files with the answer, and `job.log` holds the read-back line.
  - `a_second_review_changes_one_track_and_keeps_the_other_answer` (Review Focus 2): the job is held again by a collision, and the second approval answers only the collision.
  - In `compare_job.rs`: `a_reviewed_jobs_relayed_decide_equals_its_decide_json`: `compare-job` prints `no difference` for a job filed after a review.
  - In `ctl_engine.rs`: `review_over_the_test_pipe_files_a_held_job`, with `--answer` and `--yes`; and `discard_over_the_test_pipe_frees_a_failed_job`.
- [ ] **Step 2: Run** `CARGO test -p fake-tools --locked --offline --test engine_review --test compare_job --test ctl_engine`. Expected: PASS.
- [ ] **Step 3: Run** the whole run. Write the three numbers to the ledger.
- [ ] **Step 4: Commit** `tests: whole jobs through a review`. Then stop for prerequisite 2. On the owner's word: push, open the draft pull request with no attribution line in its body, bind it and read its CI on the three systems.

### Task 17: The owner reads one real sheet (controller, with the owner)

**Files:**
- Create: `.superpowers/pr32/` (git-ignored): the sheet as shown, and the owner's words.

- [ ] **Step 1: Ask prerequisite 3.** On yes, copy the `fama-05` job's two `sub-<n>.ocr.jsonl` files from the tuning dataset on the mini PC into `.superpowers/pr32/text/`, through a one-off task in the desktop session, as pull request 7 listed the share. On no, go on without `--text`.
- [ ] **Step 2: Print the sheet:** `target/debug/lazymkv-dev sheet <the kept fama-05 job folder> --held [--text .superpowers/pr32/text]`. The job folder is the one pull request 7 pulled.
- [ ] **Step 3: Show it to the owner** in the chat, with one question: what on this sheet is unclear or missing? Nothing else is asked.
- [ ] **Step 4: Apply the owner's words** to the sentences and the layout in `crates/core/src/review_doc.rs` and their tests, as one commit `core: the sheet's wording after the owner's reading`, and record each change in the ledger. If the owner asks for something that changes what an answer does, stop: that is a ruling for the design, not wording.
- [ ] **Step 5: Delete** `.superpowers/pr32/text/` once the commit is made.

### Task 18: Notes and close (controller)

**Files:**
- Create: `docs/phase3/review.md`
- Modify: `docs/superpowers/specs/2026-11-24-phase-3-design.md` (only where this pull request narrowed it: part 11's exit-code sentence, T32-R6)

- [ ] **Step 1: Write the notes:** what pull request 2 delivers, task by task with commits; rulings T32-R1 to T32-R9 and every T32-E; the owner's words from Task 17; what is left for pull requests 3 to 5 that this one met; the test count of CI's three jobs on the head.
- [ ] **Step 2: Run** `scripts/phase0/check-names.sh` and the whole run. Expected: exit 0, 0 failed.
- [ ] **Step 3: Commit** `docs: the notes of phase 3's pull request 2`, push, and stop for prerequisite 5.
