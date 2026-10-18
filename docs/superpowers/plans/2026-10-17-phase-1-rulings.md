# Phase 1 Rulings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the owner's rulings of 2026-10-17 on the phase 1 decision core (the open questions of `docs/phase1/decision-core.md`, the parked finding C1, the name-cleaning losses, the note spellings and the stale spec sentences) and pin the test gaps the review of pull request #3 left, in one pull request from `main` (`4cdc0ea`).

**Architecture:** Three behaviour changes in `crates/core` (rule 3.1 counts only play-all titles of episode-length parts; `S3D`/`3D` are format tags and a whole-label region tag is kept; a Java-labelled second cut TheDiscDB does not list is planned as an unnamed cut beside the main movie), each with its spec sentence; the confirmed choices and the four note names written into the spec; then tests only, except four one-line defects the test gaps exposed (`coverage`'s English test, `dts_family`'s attribute order, XML-forbidden character references, `tmdb_pending` with a given record). No new type except one `Note` variant and one `Group` predicate.

**Tech Stack:** Rust 1.99.0, edition 2021; `serde`, `serde_json`, `regex`, `unicode-normalization`, as in the workspace. No new dependency.

**Spec:** `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (sections 6.3, 8.1, 8.2, 8.3, 8.5, 8.6, 8.8, 8.9, 8.10, 10.2, 16, 18; decisions D2, D9, D15) and `docs/superpowers/specs/2026-10-13-rust-port-design.md` (RP7). The phase 1 plan `docs/superpowers/plans/2026-10-16-phase-1-decision-core.md` and `docs/phase1/decision-core.md` record the rulings RP10 to RP27 and F1 to F15 that still bind. The rulings below (OR1 to OR16, SR1 to SR9) bind every task of this plan.

**Branch:** `phase-1-rulings`, from `main` at `4cdc0ea` (the merge of pull request #3).

## Global Constraints

- Toolchain Rust 1.99.0, edition 2021; every `cargo` command runs on the mini PC through `scripts/box.sh cargo <args>` (never two at once); the evidence tests need the junction form `scripts/box.sh "(if not exist corpus mkdir corpus)&& mklink /J corpus\owner C:\lazymkv\evidence\owner&& set LAZYMKV_EVIDENCE=C:\lazymkv\evidence\owner&& cargo test --workspace --locked"`; `LAZYMKV_OCR_TEXT` is never set. CI builds and tests on Linux, macOS and Windows without the clone.
- No new dependency, no new crate, no new module file; `core` stays free of I/O (RP19).
- The one-copy rule of the phase 1 plan holds; this plan adds one shared constant (`TEXT_HINT`, Task 8) and one shared fixture row (`ENG_AC3_2CH`, Task 6) and removes their copies.
- Review reasons and notes stay closed sets with the spec's snake_case spellings; the confidences keep the spec's constants (title 0.98, 0.90, 0.85, 0.60; `auto_threshold` 0.85).
- RP21: no test in `crates/` holds a byte copy of, or a value read only from, `corpus/owner`; the pinned public logs (`corpus/thediscdb`, the fixtures) may be quoted, so Expendables 3's titles, sizes and labels are fine; `crates/dev/tests/clone_values.rs` guards it.
- Every changed public item keeps a `///` doc naming its spec section and ruling; prose in plain sentences, no em-dashes, acronyms expanded at first use.
- Tests: unit tests in the file's `mod tests` (or its sibling `tests.rs` where one exists), names in snake_case reading as sentences; tests over the clone live in `crates/dev/tests/` and skip without it.
- One commit per task; the message only, no attribution line; the implementer commits with the owner's signing configuration and checks `git log -1 --format='%G? %an <%ae> %cn <%ce>%n%B'` for `G` and `krcm0209 <17315774+krcm0209@users.noreply.github.com>`.
- Nothing under `corpus/owner/` is written; the existing goldens stay byte-identical (`reconcile`, `decide`'s first sections).
- Documents: a ruling applied by a task is written into the design spec by that task (one sentence at the cited line) and into `docs/phase1/decision-core.md` where that document describes the behaviour; Task 9 adds the rulings block and the test count.

## Rulings

Owner rulings of 2026-10-17 (OR1 to OR16; the prefix OR, owner ruling, keeps them apart from the spec's requirements R1 to R8) and the controller's sub-rulings (SR1 to SR9) made while writing this plan from the readers' findings. The owner can overturn any SR.

- OR1. 8.3 rule 3.1 counts a play-all title only when every part is episode-length (15 to 120 minutes, 8.2). OR15 limits this to rule 3.1: 8.5's dropping of play-all titles from a TV disc keeps the literal shape (any concatenation of two or more other groups' maps), so an extras play-all is never suggested as an episode. The whole group's own length is unconstrained (SR1).
- OR2. Two label cuts both named "Theatrical Cut" (the `none` and `_R` rows) keep the 0.60 `title_ambiguous` hold; two TheDiscDB main movies that would share one name likewise. No code change; the spec says so (Task 4).
- OR3. When the default audio is not English and no regular English track is kept, the SDH track is the default subtitle. No code change; the spec says so (Task 4).
- OR4. Leading periods and spaces are dropped from a path component as well as trailing ones. No code change; the spec says so (Task 4).
- OR5. C1: rule 1 still wins (TheDiscDB's main movies at 0.98, Direct), but a Java-labelled cut (rule 2's grouping) none of whose titles is a main movie's scan title is planned too, as an unnamed cut (edition none, not theatrical), with the review reason `unnamed_cut` and, per OR16, the note `unlisted_label_cut` naming the title and its label, so the owner names it with Assign titles. The main movie's own label suffix, its dub variants and a Narrative variant name no second cut (the Task 5 ruling stands).
- OR6. Name cleaning: `S3D` and `3D` are format tags (never a season marker or a disc number); a region tag that is the whole label is kept (`US` is the film Us). SR2 settles the scope: the whole-label rule applies to a single-token label that is a region tag (`US`, `UK`, `EU`, `WW`, `DOM`, `INT`, `R1` to `R6`), with its brackets stripped (`(US)` gives `US`); a format tag that is the whole label (`UHD`, `DVD`, `BLU-RAY`, `3D`) still cleans to nothing and stays generic; a label of two or more tag tokens cleans to nothing. `S1M0NE`, `BLADE_RUNNER_2049` and `D2_THE_MIGHTY_DUCKS` stay as they are.
- OR7. The object-audio spellings (`Atmos`, `DTS:X`) stay provisional until experiment 6 runs. No change.
- OR8. RP27's four note names are confirmed: `possible_audio_description`, `lossy_before_lossless`, `direct_mode_fallback`, `label_rule_unavailable`; the spec names them (Task 4).
- OR9. RP25's subtitle-label vocabulary is confirmed. No change.
- OR10. The final review's six provisional rules are kept. No change.
- OR11. RP10 is confirmed; the spec's three sentences still saying "phase 1" for phase 2 work (lines 226, 363, 743) are edited now (Task 4).
- OR12. Of the 95 deferred minors only the test gaps are pinned (Tasks 5 to 8); where a gap exposed a defect, the defect is fixed with the test (SR4, SR5, SR6, SR8).
- OR13. The four phase 2 leftovers (lookup answers outside AwaitingLookup, restart between answer and refresh, collision answer by path, a relative root in `decide`) stay for phase 2.
- OR14. The pull request runs subagent-driven.
- OR15. (Scope of OR1.) The episode-length restriction applies to rule 3.1 only; 8.5 keeps dropping any concatenation title.
- OR16. (Form of OR5's note.) A new note `unlisted_label_cut`, the thirteenth of the closed set, with the title and label in its detail, carried into `Plan.notes` and every `FileDecision.notes`; plus an evidence sentence.
- SR1. Rule 3.1's parts are each checked against the 8.2 band; the whole group's length and the parts' mutual spread are not checked.
- SR2. The whole-label rule's scope, as written under OR6.
- SR3. `by_database` takes `wanted: &Wanted` (the languages `movie_plan` already holds) so the unlisted cut's variant is chosen as rule 2 chooses it; no second pass with the original language for rule 1 plans.
- SR4. `plan::movie::coverage` uses `lang::is_english` (phase 1 plan decision 6: one English test); `Wanted.english` stays, serialised as before.
- SR5. `codec::dts_family` reads attribute 6, then attribute 7, as its doc says (a conflicting made-up pair follows attribute 6); the DTS:X pre-check stays first.
- SR6. `disc::character_reference` refuses a numeric reference outside XML 1.0's Char production (U+0000 to U+0008, U+000B, U+000C, U+000E to U+001F, U+FFFE, U+FFFF, and the surrogates and values above U+10FFFF it already refuses), leaving it as written like a refused entity.
- SR7. Pinned as they are, no change: a tuning root inside the staging root is refused whatever `keep_tuning_evidence` says; owner-assigned episodes join or create the season session under an unconfirmed search identity (the owner assigned them); ffprobe's object extension wins over attribute 7's; a DTS:X track is named `DTS-HD MA DTS:X 7.1` (display name, extension, layout, as `TrueHD Atmos 7.1`).
- SR8. `lazymkv-dev plan`: a `--tmdb-record` file clears the pending flag a failed TMDb lookup set (the option replaces the recording's record); `--tmdb-search` does not (a different lookup); a failed TheDiscDB lookup keeps the job pending.
- SR9. `TEXT_HINT` has one definition, `pub const` in `core::subtitle::labels` (its harness text already carries it); the dev crate uses it.

## Review Focus

1. **A TV disc whose only kind evidence was an extras play-all.** Under OR1 a disc without TheDiscDB, labels or a season marker, whose episodes are two features' worth of rivals and whose play-all is of short extras, is no longer TV: rule 3 runs and holds it at 0.60 in copy mode. Task 1's `a_tv_disc_known_only_by_an_extras_play_all_is_held_as_a_movie_in_copy_mode` pins it on CHERNOBYL_2 without TheDiscDB (rule Structure, 0.60, `multiple_features` or `title_ambiguous` as the mini PC run shows; the implementer reports which).
2. **A region tag in another case or in brackets as the whole label.** `(us)` and `us` are the film Us too. Task 2's `a_region_tag_that_is_the_whole_label_is_kept` pins `us`, `(US)` and `UK`, and that `US UK` and `UHD` still clean to nothing.
3. **A second cut present only as a dub variant.** A disc whose main movie is `FPL_MainFeature_UR` and whose only theatrical label is `FPL_MainFeature_R_JPN` has an unlisted cut (the `_R` cut, chosen title the Japanese variant). Task 3's rule 1 and rule 2 interaction test pins it.
4. **A numeric character reference XML forbids inside a disc name.** `A&#0;B` must not become a name holding U+0000 (a path component with a NUL). Task 5's `bdmt_name_does_not_decode_a_character_xml_forbids` pins it.
5. **A record given while both lookups failed.** With `tmdb movie/N 500` and a TheDiscDB failure, `--tmdb-record` clears only the TMDb half: the plan stays pending through `discdb_pending`. Task 8's test pins both halves.

---

## File structure

| Path | Change |
|---|---|
| `crates/core/src/plan/kind.rs` | `is_episode_length`, `play_all_of_episodes`, the part predicate inside `concatenates`; tests. |
| `crates/core/src/plan/movie.rs` | `by_structure` uses `play_all_of_episodes`; `by_database(record, matches, disc, wanted)` plans unlisted labelled cuts (OR5); `coverage` uses `is_english` (SR4); tests. |
| `crates/core/src/plan/mod.rs` | Docs of `PlannedRole`, `movie_entries`; the `by_database` call; tests (rule 3.1, C1, test gaps T7). |
| `crates/core/src/plan/tv.rs` | Fixture row removed (T6); `without_play_all` test. |
| `crates/core/src/review.rs` | `Note::UnlistedLabelCut`; header comments; tests. |
| `crates/core/src/lookup/name.rs` | `REGION_TAGS`, `FORMAT_TAGS`, `Piece.tag`, the marker guard, the whole-label rule; tests. |
| `crates/core/src/lookup/tests.rs` | T4 test. |
| `crates/core/src/lang.rs` | No change (SR4 lives in movie.rs). |
| `crates/core/src/codec.rs` | `dts_named`, `dts_family` order (SR5); tests. |
| `crates/core/src/settings.rs` | T1 test, doc line. |
| `crates/core/src/disc.rs` | XML Char check in `character_reference` (SR6); test helpers tidy; tests. |
| `crates/core/src/remembered.rs` | Fixture rows removed (T6). |
| `crates/core/src/tracks/audio/tests.rs` | T8 tests. |
| `crates/core/src/tracks/subtitle/tests.rs` | T9, T10 tests. |
| `crates/core/src/tracks/layout.rs` | Doc comment (OR3); OR3 layout-level test; T11 test rows. |
| `crates/core/src/naming.rs` | Doc comment (OR4). |
| `crates/core/src/verify.rs` | T12 tests. |
| `crates/core/src/decision.rs` | Doc of `job_features`; T13 tests. |
| `crates/core/src/job/tests.rs` | T14 sample. |
| `crates/core/src/subtitle/labels.rs`, `labels/row14.rs`, `row14/tests.rs` | `pub const TEXT_HINT` (SR9); T18a test. |
| `crates/dev/src/cmd/plan.rs` | `tmdb_pending` fix (SR8); test. |
| `crates/dev/src/cmd/retune.rs`, `cmd/decide.rs`, `cmd/golden.rs` | Use `TEXT_HINT` from core. |
| `crates/dev/tests/plan.rs` | Expendables expectation (OR5); `cut` helper. |
| `crates/dev/tests/labels_subtitles.rs` | T17 tests. |
| `crates/dev/tests/decide.rs` | Uses `TEXT_HINT` from core. |
| `docs/superpowers/specs/2026-10-04-lazymkv-design.md` | Lines 36, 45, 197, 201, 226, 246, 260, 270, 291, 314, 349-355, 363, 397-407, 443, 447, 571, 700, 743 (one sentence each, cited per task). |
| `docs/phase1/decision-core.md` | Row 131; rulings 290, 323-325, 387-389; the "Choices" bullets 362-381; RP27 284-286; the rulings block; the test count. |
| `README.md` | Lines 22-23. |

## Task index

| Task | Deliverable | Model tier |
|---|---|---|
| 1 | Rule 3.1 counts only play-all titles of episode-length parts (OR1, OR15, SR1) | standard |
| 2 | `S3D`/`3D` are format tags; a whole-label region tag is kept (OR6, SR2) | standard |
| 3 | C1: an unlisted labelled cut beside the main movie (OR5, OR16, SR3) | standard |
| 4 | The confirmed choices and the note names written into the spec (OR2, OR3, OR4, OR7, OR8, OR9, OR11) | cheap |
| 5 | Core test gaps A: lang, codec, settings, disc (A4, T1, T2; SR4, SR5, SR6) | standard |
| 6 | Core test gaps B: the planner (T4, T5, T6, T7, T8; SR7) | standard |
| 7 | Core test gaps C: subtitles, layout, verify, decision, job (T9 to T14) | standard |
| 8 | Dev test gaps (T15, T17, T18; SR8, SR9) | standard |
| 9 | Closing documents and the full run (rulings block, test count, README) | cheap |

Every `scripts/box.sh` run in the steps below is the junction form from Global Constraints when the test reads the clone (Tasks 3, 8, 9); the plain `scripts/box.sh cargo test -p lazymkv-core <filter>` otherwise.

---

### Task 1: Rule 3.1 counts only play-all titles of episode-length parts

**Files:**
- Modify: `crates/core/src/plan/kind.rs:1-4` (module doc), `:104-115` (band), `:129-165` (`play_all`, `concatenates`), tests `:564-633`
- Modify: `crates/core/src/plan/movie.rs:411-426` (`StructureOutcome` doc, `by_structure`), tests `:1361-1382`, `:1473-1511`
- Modify: `crates/core/src/plan/mod.rs:207-211` (doc), `:2066-2071` (comment), tests
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md:291`, `:314`
- Modify: `docs/phase1/decision-core.md:323-325`, `:366-370`

**Interfaces:**
- Consumes: `kind::Group { key, segment_map, segments, titles, seconds, truncated }`, `EPISODE_MIN_S`, `EPISODE_MAX_S`, `kind::play_all(groups, dvd) -> Option<usize>` (unchanged, 8.5's reading), `movie::by_structure(disc, groups, settings) -> StructureOutcome`.
- Produces: `pub fn is_episode_length(group: &Group) -> bool`; `pub fn play_all_of_episodes(groups: &[Group], dvd: bool) -> Option<usize>` (8.3 rule 3.1's reading: every part passes `is_episode_length`); `movie::by_structure` answers `PlayAll` only from `play_all_of_episodes`. `tv::without_play_all` keeps `play_all`.

- [ ] **Step 1: Write the failing tests in `kind.rs`**

In `mod tests`, with the helpers `bd(map, titles, seconds)` and `groups_of(disc)`:

```rust
#[test]
fn a_play_all_of_episodes_counts_only_parts_of_episode_length() {
    let two_parts = |a: u32, b: u32| vec![bd("1", &[1], a), bd("2", &[2], b), bd("1,2", &[3], a + b)];
    assert_eq!(play_all_of_episodes(&two_parts(1320, 1320), false), Some(2));
    assert_eq!(play_all_of_episodes(&two_parts(900, 7200), false), Some(2), "the band is inclusive");
    assert_eq!(play_all_of_episodes(&two_parts(1320, 600), false), None, "a ten-minute part");
    assert_eq!(play_all_of_episodes(&two_parts(899, 1320), false), None);
    assert_eq!(play_all_of_episodes(&two_parts(7201, 1320), false), None);
    assert_eq!(play_all_of_episodes(&two_parts(1320, 1320), true), None, "never on a DVD");
    // 8.5's reading (OR15) is unchanged: any parts.
    assert_eq!(play_all(&two_parts(1320, 600), false), Some(2));
}

#[test]
fn a_whole_with_only_an_extras_decomposition_is_a_play_all_for_8_5_but_not_for_rule_3_1() {
    let groups = vec![bd("1", &[1], 1320), bd("5", &[2], 300), bd("1,5", &[3], 1620)];
    assert_eq!(play_all(&groups, false), Some(2));
    assert_eq!(play_all_of_episodes(&groups, false), None);
}

#[test]
fn is_episode_length_is_15_to_120_minutes_inclusive() {
    for (seconds, expected) in [(899, false), (900, true), (7200, true), (7201, false)] {
        assert_eq!(is_episode_length(&bd("1", &[1], seconds)), expected, "{seconds}");
    }
}
```

In the existing `the_pinned_logs_play_all_groups`, after each of the three logs whose `play_all` index is `Some` (CHERNOBYL_1, CHERNOBYL_2, SMILE), add `assert_eq!(play_all_of_episodes(&found, false), None, "{log}: its parts are short extras");` where `found` is the groups the test already builds.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core plan::kind`
Expected: compile error, `play_all_of_episodes` and `is_episode_length` not found.

- [ ] **Step 3: Implement the predicate and the two readings in `kind.rs`**

- `pub fn is_episode_length(group: &Group) -> bool`: `(EPISODE_MIN_S..=EPISODE_MAX_S).contains(&group.seconds)`; `episode_length_groups` filters with it.
- `pub fn play_all(groups, dvd)` keeps its signature and doc (now "8.5's play-all title, any parts") and calls a private `fn play_all_where(groups: &[Group], dvd: bool, part: &dyn Fn(&Group) -> bool) -> Option<usize>`, the current body.
- `pub fn play_all_of_episodes(groups, dvd)`: `play_all_where(groups, dvd, &is_episode_length)`; doc: "Spec 8.3 rule 3.1 under ruling OR1: a play-all title whose every part is episode-length (15 to 120 minutes, 8.2); a concatenation of shorter extras is not one. Def: this plan. Use: `movie::by_structure`."
- `concatenates` gains the `part` argument; its skip condition gains `|| !part(group)`. Module doc line 4 says the two readings.

- [ ] **Step 4: Run the kind tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core plan::kind`
Expected: PASS, every existing play-all test unchanged.

- [ ] **Step 5: Write the failing tests in `movie.rs` and `plan/mod.rs`**

`movie.rs` (helpers `structure_disc`, `groups_of`, `structure`, `pinned`, `feature`):

```rust
#[test]
fn a_play_all_of_episode_length_parts_makes_the_structure_rule_answer_tv() {
    let disc = structure_disc(&[
        (0, "1:07:30", "15", "1,2,3", &[ENG_AC3]),
        (1, "0:22:00", "5", "1", &[ENG_AC3]),
        (2, "0:22:30", "5", "2", &[ENG_AC3]),
        (3, "0:23:00", "5", "3", &[ENG_AC3]),
    ]);
    let settings = Settings::default();
    assert_eq!(by_structure(&disc, &groups_of(&disc, &settings), &settings), StructureOutcome::PlayAll { group: 0 });
}

#[test]
fn an_extras_play_all_is_not_rule_3_1s_play_all_title() {
    // OR1: the pinned logs' play-all titles (a deleted-scenes playlist, two extras playlists) have parts of a few minutes.
    for log in [CHERNOBYL_1, CHERNOBYL_2, SMILE] {
        let disc = pinned(log);
        let settings = Settings::default();
        assert!(matches!(by_structure(&disc, &groups_of(&disc, &settings), &settings), StructureOutcome::Plan(_)), "{log}");
    }
    let disc = structure_disc(&[
        (0, "1:40:00", "16", "1", &[ENG_AC3]),
        (1, "0:05:00", "", "2", &[ENG_AC3]),
        (2, "0:05:00", "", "3", &[ENG_AC3]),
        (3, "0:10:00", "", "2,3", &[ENG_AC3]),
    ]);
    let plan = structure(&disc, &Settings::default());
    assert_eq!((plan.features, plan.confidence), (vec![feature(0, None, true, &[])], 0.85));
}

#[test]
fn a_tv_disc_known_only_by_an_extras_play_all_is_held_as_a_movie_in_copy_mode() {
    // Review Focus 1: without TheDiscDB, labels or a season marker, CHERNOBYL_2's extras play-all no longer makes it TV.
    let disc = pinned(CHERNOBYL_2);
    let settings = Settings::default();
    let found = groups_of(&disc, &settings);
    match movie_plan(&disc, None, &BTreeMap::new(), &found, &wanted_languages(None), &settings) {
        StructureOutcome::Plan(plan) => {
            assert_eq!((plan.rule, plan.confidence), (TitleRule::Structure, 0.60));
            assert!(!plan.reviews.is_empty(), "{:?}", plan.reviews);
        }
        other => panic!("{other:?}"),
    }
}
```

Delete `a_play_all_group_makes_the_structure_rule_answer_tv`. In `movie_plan_takes_the_first_applicable_rule`, replace the CHERNOBYL_1 `PlayAll { group: 1 }` assertion by `assert_eq!(rule_of(movie_plan(&chernobyl, None, &BTreeMap::new(), &chernobyl_groups, &english, &settings)), Some(TitleRule::Structure));`. `a_feature_whose_segments_are_listed_as_playlists_is_play_all_under_the_literal_reading` stays (its parts are 33 minutes).

`plan/mod.rs`, built exactly as `a_play_all_title_makes_a_movie_disc_tv_and_is_never_suggested` builds its inputs (a name-only lookup, `Settings::default()`, `LibraryFacts::default()`):

```rust
#[test]
fn an_extras_play_all_on_a_disc_without_thediscdb_leaves_it_a_movie() {
    let disc = made_disc(false, "EXAMPLE_FILM", &[
        made(1, "1:40:00", "16", "00001.mpls", "1", "30000000000", "", ENG),
        made(2, "0:05:00", "2", "00002.mpls", "2", "1500000000", "", ENG),
        made(3, "0:05:00", "2", "00003.mpls", "3", "1500000000", "", ENG),
        made(4, "0:10:00", "4", "00004.mpls", "2,3", "3000000000", "", ENG),
    ]);
    let plan = /* run(&inputs(..)) as the neighbouring test does */;
    assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
    assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
    assert_eq!(plan.titles.iter().map(|planned| planned.title).collect::<Vec<_>>(), vec![1]);
    assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
    assert!(!plan.evidence.sentences.iter().any(|sentence| sentence.starts_with("8.3 rule 3.1")), "{:?}", plan.evidence.sentences);
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core plan::`
Expected: the three new movie tests and the plan test FAIL (PlayAll where Plan is expected); the deleted test is gone.

- [ ] **Step 7: Switch `by_structure` and update the docs and comments**

- `movie::by_structure`: `play_all_of_episodes(groups, disc.is_dvd)`; its doc and `StructureOutcome::PlayAll`'s doc say "rule 3.1 under OR1: a play-all title of episode-length parts".
- `plan/mod.rs:207-211` doc and the comment at `:2066-2071`: the 10-minute credits clip is not episode-length, so the group is not rule 3.1's play-all title; rule 3 runs directly, as TheDiscDB's movie kind would make it anyway.
- Spec line 291 becomes: `1. If a group's segment map is the concatenation of two or more other groups' maps, each of them episode-length (15 to 120 minutes, 8.2), it is a "play all" title: treat the disc as TV (8.5). A concatenation of shorter groups (a play-all of extras) is an ordinary group here (ruling OR1, 2026-10-17).`
- Spec line 314, after `"play all" titles are dropped`: `(here any group whose segment map is the concatenation of two or more other groups' maps, whatever their length: a play-all of extras is dropped too; OR15)`.
- `decision-core.md:366-370`: the open question becomes `- Ruling OR1 (2026-10-17): 8.3 rule 3.1 counts a play-all title only when every part is episode-length (15 to 120 minutes); a play-all of extras (Smile's deleted-scenes playlist, title 117) is an ordinary group, so a movie disc with one stays a movie without TheDiscDB. 8.5 still drops any concatenation title from a TV disc (OR15).` and `:323-325` (F4) ends `; ruling OR1 then settled it.`

- [ ] **Step 8: Run the core tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: PASS. Report in the task report which review reason Review Focus 1's test found.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/plan docs/superpowers/specs/2026-10-04-lazymkv-design.md docs/phase1/decision-core.md
git commit -m "core::plan: rule 3.1 counts only play-all titles of episode-length parts (OR1, OR15); 8.5 keeps the literal shape"
```

---

### Task 2: `S3D` and `3D` are format tags; a whole-label region tag is kept

**Files:**
- Modify: `crates/core/src/lookup/name.rs:7-12` (module doc), `:18-57` (`Piece`, `pieces`), `:80-104` (`markers`), `:124-128` (`has_season_marker` doc), `:130-144` (tags), `:154-193` (`clean`), tests `:474-492`, `:524-532`, `:551-559`
- Modify: `crates/core/src/plan/kind.rs` tests (one test beside `a_season_marker_in_the_disc_name_makes_a_tv_disc`)
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md:246`, `:260`

**Interfaces:**
- Consumes: `tokens`, `pieces`, `markers`, `first_marker`, `is_tag`, `unbracketed`, `clean`, `is_generic`, `parse_season`, `parse_disc_number`, `has_season_marker`.
- Produces: `const REGION_TAGS: [&str; 12]` (`US`, `UK`, `EU`, `WW`, `DOM`, `INT`, `R1` to `R6`), `const FORMAT_TAGS: [&str; 14]` (the other twelve of today's list plus `S3D`, `3D`), `fn is_region_tag(token: &str) -> bool`, `is_tag` = either; `Piece.tag: bool`.

- [ ] **Step 1: Write the failing tests**

Replace `a_3d_label_s3d_reads_as_season_3_by_spec_8_2s_literal_rule` with:

```rust
#[test]
fn s3d_and_3d_are_format_tags_not_a_season_marker() {
    // OR6: `S3D` (stereoscopic 3D) is a format tag, so its pieces S, 3, D belong to no marker.
    assert_eq!(parse_season("HUGO_S3D"), None);
    assert_eq!(parse_disc_number("HUGO_S3D"), None);
    assert!(!has_season_marker("HUGO_S3D"));
    assert_eq!(clean("HUGO_S3D"), "HUGO");
    assert_eq!(clean("AVATAR 3D"), "AVATAR");
    assert_eq!(clean("HUGO-S3D"), "HUGO");
    assert_eq!(clean("HUGO (3D)"), "HUGO");
    assert_eq!(parse_disc_number("AVATAR 3D 2"), None, "the D of 3D does not reach across the token boundary");
    // `S3D3` is not a tag: season 3, disc 3, as before.
    assert_eq!((parse_season("EXAMPLE SHOW S3D3"), parse_disc_number("EXAMPLE SHOW S3D3")), (Some(3), Some(3)));
    assert_eq!(clean("EXAMPLE SHOW S3D3"), "EXAMPLE SHOW");
}

#[test]
fn a_region_tag_that_is_the_whole_label_is_kept() {
    // OR6 and SR2: a disc labelled US is the film Us; a format tag alone is still a placeholder.
    assert_eq!(clean("US"), "US");
    assert_eq!(clean("us"), "us");
    assert_eq!(clean("(US)"), "US");
    assert_eq!(clean("UK"), "UK");
    assert_eq!(clean("US UK"), "");
    assert_eq!(clean("UHD"), "");
    assert_eq!(clean("BLU-RAY"), "");
    assert!(!is_generic("US", &clean("US")));
    assert!(is_generic("UHD", &clean("UHD")));
    // a region tag beside other words is still removed
    assert_eq!(clean("THE UK FILES"), "THE FILES");
}
```

In `every_region_and_format_tag_is_removed_as_a_whole_token_in_any_case`: add `"S3D", "3D"` to its `tags` array; change `assert_eq!(clean("US"), "");` to `assert_eq!(clean("US"), "US");` and reword the comment (a tag token beside other words is removed; alone, a region tag is the name: OR6). In `a_name_with_no_letters_or_nothing_left_after_cleaning_is_generic`: replace the last two lines by `assert!(!is_generic("US", &clean("US")), "the film Us is searched (OR6)");`.

In `plan/kind.rs` tests, beside `a_season_marker_in_the_disc_name_makes_a_tv_disc` and built the same way with the name `HUGO_S3D` and a movie's groups: `a_3d_label_is_not_a_season_marker_for_the_kind`, asserting `(Kind::Movie, KindSource::Default)`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core lookup::name`
Expected: FAIL on `parse_season("HUGO_S3D")` (Some(3)) and `clean("US")` ("").

- [ ] **Step 3: Implement**

- Split `TAGS` into `REGION_TAGS` and `FORMAT_TAGS` (docs name spec 8.1 and OR6); `is_region_tag(token)` and `is_tag(token)` (`is_region_tag || FORMAT_TAGS.contains(..)`), both through `unbracketed` and ASCII upper-case as today.
- `Piece` gains `tag: bool`; `pieces` sets it to `is_tag(word) || is_tag(text)` for every piece of that token and hyphen part.
- `markers`: the condition at `:86` becomes `!word.digits && next.digits && !word.tag && !next.tag`.
- `clean`: before `kept.join(" ")`, `if kept.is_empty() && tokens.len() == 1 && is_region_tag(&tokens[0]) { return unbracketed(&tokens[0]).to_string(); }`.
- Docs: module doc (a piece of a tag token or tag part belongs to no marker), `clean`'s doc (the tag lists with `S3D` and `3D`; the whole-label sentence), `has_season_marker`'s doc (`S3D` and `3D` are never a season marker), and remove the "listed for the owner" remarks at `:488`, `:526-528`, `:557`.
- Spec line 246, after "region and format tags", a parenthesis: (`3D` and `S3D`, stereoscopic, are format tags; a region tag that is the whole name is kept, since a disc labelled `US` is the film Us: ruling OR6, 2026-10-17). Spec line 260, after "(`season`, `series` or `s` followed by a number)": "; `S3D` and `3D` are format tags, not season markers (OR6)".

- [ ] **Step 4: Run the core tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: PASS (the thirteen-name tables, `S3D3` and `S4D1` tests unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/lookup/name.rs crates/core/src/plan/kind.rs docs/superpowers/specs/2026-10-04-lazymkv-design.md
git commit -m "core::lookup::name: S3D and 3D are format tags, a whole-label region tag is kept (OR6)"
```

---

### Task 3: C1: a Java-labelled cut TheDiscDB does not list is an unnamed cut beside the main movie

**Files:**
- Modify: `crates/core/src/review.rs:1-7`, `:104-172` (`Note`), tests `:265-275`, `:315-336`
- Modify: `crates/core/src/plan/movie.rs:1-7`, `:154-167` (`Feature.theatrical` doc), `:185-253` (`by_database`), `:597-606` (`movie_plan`), tests `:797-801` (`database` helper), `:803-821`, `:1584-1593`, new test after `:922`
- Modify: `crates/core/src/plan/mod.rs:97-109` (`PlannedRole` doc), `:656-659` (`movie_entries` doc), tests `:1377-1395`
- Modify: `crates/core/src/decision.rs:455-470` (`job_features` doc)
- Modify: `crates/dev/tests/plan.rs:143-145` (add `cut`), `:188-200`
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md:270`
- Modify: `docs/phase1/decision-core.md:131`, `:290`, `:387-389`

**Interfaces:**
- Consumes: `by_labels(disc, wanted) -> Option<MoviePlan>` (rule 2's cuts: `features[i].title` the chosen variant, `.siblings` the cut's other titles, `feature_seconds[i]`), `Feature`, `MoviePlan`, `Noted { note, stream, detail }`, `sorted_unique`, `Wanted`.
- Produces: `Note::UnlistedLabelCut` (`"unlisted_label_cut"`, last in the enum, `ALL: [Note; 13]`); `pub fn by_database(record: &DiscDbRecord, matches: &BTreeMap<u32, u32>, disc: &Disc, wanted: &Wanted) -> Option<MoviePlan>` whose features are the main movies in database index order, then each unlisted cut (`Feature { title: <the cut's chosen title>, edition: None, theatrical: false, siblings }`) in rule 2's cut order; `reviews` gains `UnnamedCut` when an unlisted cut exists; `confidence` stays as computed from the main movies alone (0.98 without main-movie reasons); `notes` holds one `Noted { note: UnlistedLabelCut, stream: None, detail }` per unlisted cut; `evidence` gains one sentence per unlisted cut. Exact texts: evidence `scan title {n} carries the label {comment}, a cut TheDiscDB does not list as a main movie`; detail `title {n}'s label {comment} names a cut TheDiscDB does not list; name it with Assign titles`, where `{comment}` is the chosen title's attribute 49 as written.

- [ ] **Step 1: Write the failing tests**

`review.rs`: a `round_trips!` row `note_unlisted_label_cut_round_trips` for `Note::UnlistedLabelCut` / `"unlisted_label_cut"`; rename `all_notes_are_the_twelve_in_spec_order` to `all_notes_are_the_thirteen_in_spec_order` and append `"unlisted_label_cut"` to its list.

`movie.rs`, after the `by_database` block (helpers `record`, `java_disc`, `two_titles`, `feature`, `wanted_languages`, fixtures `ENG_AC3`, `JPN_AC3`):

```rust
#[test]
fn a_java_label_naming_a_cut_thediscdb_does_not_list_is_an_unnamed_cut_beside_the_main_movie_at_0_98() {
    // OR5: rule 1 still wins, and the labelled cut TheDiscDB does not list is ripped as an unnamed cut for Assign titles.
    let one = record(Some("Film"), &["Film"]);
    let main_is_title_1 = BTreeMap::from([(0, 1)]);
    let english = wanted_languages(None);
    let disc = java_disc(&[(1, "FPL_MainFeature_UR", &[ENG_AC3]), (2, "FPL_MainFeature_R", &[ENG_AC3])]);
    let plan = by_database(&one, &main_is_title_1, &disc, &english).expect("rule 1 applies");
    assert_eq!(plan.rule, TitleRule::Database);
    assert_eq!(plan.features, vec![feature(1, Some("Unrated Cut"), false, &[]), feature(2, None, false, &[])]);
    assert_eq!(plan.feature_seconds, vec![Some(6000), Some(6000)]);
    assert_eq!((plan.confidence, plan.reviews.clone()), (0.98, vec![ReviewReason::UnnamedCut]));
    assert_eq!(
        plan.notes,
        vec![Noted {
            note: Note::UnlistedLabelCut,
            stream: None,
            detail: "title 2's label FPL_MainFeature_R names a cut TheDiscDB does not list; name it with Assign titles".to_string(),
        }]
    );
    assert_eq!(
        plan.evidence.last().map(String::as_str),
        Some("scan title 2 carries the label FPL_MainFeature_R, a cut TheDiscDB does not list as a main movie")
    );
    // Review Focus 3: a cut present only as a dub variant is unlisted too, its chosen title the variant.
    let dub_only = java_disc(&[(1, "FPL_MainFeature_UR", &[ENG_AC3]), (2, "FPL_MainFeature_R_JPN", &[JPN_AC3])]);
    let plan = by_database(&one, &main_is_title_1, &dub_only, &english).expect("rule 1 applies");
    assert_eq!(plan.features, vec![feature(1, Some("Unrated Cut"), false, &[]), feature(2, None, false, &[])]);
    // The Task 5 ruling stands: the main movie's own suffix, its dub variant and a Narrative variant name no second cut.
    let own = java_disc(&[
        (1, "FPL_MainFeature_DC", &[ENG_AC3]),
        (2, "FPL_MainFeature_DC_JPN", &[JPN_AC3]),
        (3, "FPL_MainFeature_Narrative", &[ENG_AC3]),
    ]);
    let plan = by_database(&one, &main_is_title_1, &own, &english).expect("rule 1 applies");
    assert_eq!(plan.features, vec![feature(1, None, true, &[])]);
    assert_eq!((plan.confidence, plan.reviews, plan.notes), (0.98, vec![], vec![]));
    // Labels are trusted only when Java is in use (9.4): without MSG:3344 the labelled title 0 is not a second cut.
    let plain = by_database(&one, &main_is_title_1, &two_titles(false, "FPL_MainFeature_R"), &english).expect("rule 1 applies");
    assert_eq!((plain.features, plain.reviews), (vec![feature(1, None, true, &[])], vec![]));
    // An unlisted cut never makes title_ambiguous: it has no name to share.
    let unnamed_main = record(Some("Film"), &["Film"]);
    let plan = by_database(&unnamed_main, &main_is_title_1, &java_disc(&[(1, "FPL_MainFeature", &[ENG_AC3]), (2, "FPL_MainFeature_R", &[ENG_AC3])]), &english).expect("rule 1 applies");
    assert_eq!((plan.confidence, plan.reviews), (0.98, vec![ReviewReason::UnnamedCut]));
}
```

Update the Expendables expectations (the public pinned log):
- `expendables_main_movie_takes_its_edition_from_the_label_table` (rename `..._and_its_r_label_is_an_unlisted_unnamed_cut`): features `[feature(41, Some("Unrated Cut"), false, &[]), feature(54, None, false, &[])]`, `feature_seconds` `[Some(7891), Some(7587)]`, confidence `0.98`, reviews `[ReviewReason::UnnamedCut]`, notes `[Noted { note: Note::UnlistedLabelCut, stream: None, detail: "title 54's label FPL_MainFeature_R names a cut TheDiscDB does not list; name it with Assign titles".into() }]`, evidence gains the third line `scan title 54 carries the label FPL_MainFeature_R, a cut TheDiscDB does not list as a main movie`.
- `expendables_editions_are_named_from_its_plans`: the database half gives `names(&[Some("Unrated Cut"), None])`.
- `plan/mod.rs` `expendables_3_plans_its_main_movie_as_the_unrated_cut`: titles `[planned("t00-00756", 41, feature(Some("Unrated Cut")), 7891, 28_831_358_976, Some("00756.mpls")), planned("t01-00820", 54, cut(None), 7587, 27_736_528_896, Some("00820.mpls"))]`; `(0.98, Mode::Direct)` unchanged; reviews `[ReviewReason::UnnamedCut]`; notes the one `Noted` above; reword the comment.
- `crates/dev/tests/plan.rs` `expendables_3_plans_title_41_as_the_unrated_cut`: add `fn cut(edition: Option<&str>) -> PlannedRole { PlannedRole::Feature { edition: edition.map(str::to_string), theatrical: false } }` beside `feature`; the same two titles; replace `assert_filed_automatically` by `(0.98, Mode::Direct)`, reviews `[ReviewReason::UnnamedCut]`, and the notes' `(note, stream)` pairs `[(Note::UnlistedLabelCut, None)]`.
- The `database(log, hash)` helper in `movie.rs` and every other `by_database` call pass `&wanted_languages(None)`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core review:: plan::`
Expected: compile errors (`UnlistedLabelCut`, the fourth argument).

- [ ] **Step 3: Implement**

- `review.rs`: the variant with doc `Spec 8.3 rule 1 under ruling OR5 (2026-10-17): a Java label names a cut TheDiscDB does not list as a main movie; names the title and its label.`; `ALL`, `as_str`; the header comment now says the four spellings were confirmed (OR8) and this plan adds `unlisted_label_cut` (OR16).
- `movie.rs` `by_database(record, matches, disc, wanted)`: after `same_name` and the main-movie `reviews` and `confidence` (unchanged), collect `main_titles: BTreeSet<u32>` from `features`; `if let Some(labelled) = by_labels(disc, wanted)`, for each `(cut, seconds)` of `labelled.features` zipped with `labelled.feature_seconds` whose `title` and `siblings` are all outside `main_titles`: push the evidence sentence, the `Noted`, the seconds and `Feature { title: cut.title, edition: None, theatrical: false, siblings: cut.siblings }`; then `if !unlisted.is_empty() { reviews.push(ReviewReason::UnnamedCut); }`, extend `features`, return `reviews: sorted_unique(reviews), notes`. Imports: `Note`, `BTreeSet`. `movie_plan` passes its `wanted`. Docs: `by_database` (OR5), `Feature.theatrical` ("or a Java-labelled cut TheDiscDB does not list"), module doc.
- `plan/mod.rs` docs: `PlannedRole::Feature.theatrical` (false also for rule 1's unlisted cut); `movie_entries` ("database index order for rule 1, then each unlisted labelled cut"). `decision.rs` `job_features` doc likewise. No code change in either.
- Spec line 270 becomes: "1. **Database.** TheDiscDB marks the main movie and it reconciles with the scan: title confidence 0.98. When the scan reported a Java runtime in use, a labelled cut (rule 2's grouping) none of whose titles is a main movie's scan title is planned too, as an unnamed cut, with the review reason `unnamed_cut` and the note `unlisted_label_cut` naming the title and its label, so the owner names it with Assign titles; the title confidence stays 0.98 (ruling OR5, 2026-10-17; D2)."
- `decision-core.md:131`: titles `title 41 (00756.mpls), edition `Unrated Cut` (from the Java label table, `FPL_MainFeature_UR`), then title 54 (00820.mpls, `FPL_MainFeature_R`) as an unnamed cut (OR5)`; Reviews `unnamed_cut`; note `unlisted_label_cut`. `:290` gains `Ruling OR5 (2026-10-17) adds: a Java-labelled cut none of whose titles is a main movie's is planned as an unnamed cut with unnamed_cut and the note unlisted_label_cut; the confidence stays 0.98.` `:387-389` gains the same sentence in its own words.

- [ ] **Step 4: Run the workspace tests with the clone to verify they pass**

Run: the junction form of `scripts/box.sh` (Global Constraints)
Expected: PASS, including `crates/dev/tests/plan.rs` and `clone_values.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/review.rs crates/core/src/plan crates/core/src/decision.rs crates/dev/tests/plan.rs docs/superpowers/specs/2026-10-04-lazymkv-design.md docs/phase1/decision-core.md
git commit -m "core::plan::movie: a Java-labelled cut TheDiscDB does not list is an unnamed cut beside the main movie at 0.98 (OR5, OR16)"
```

---

### Task 4: The confirmed choices and the note names written into the spec

**Files:**
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md:36`, `:45`, `:197`, `:201`, `:226`, `:349-355`, `:363`, `:397-407`, `:443`, `:447`, `:571`, `:700`, `:743`
- Modify: `crates/core/src/tracks/layout.rs:103-106` (doc), tests beside `:587`
- Modify: `crates/core/src/naming.rs:57-63` (doc)
- Modify: `docs/phase1/decision-core.md:284-286` (RP27), `:371-381` (open questions 2 to 4), `:398-399` (object audio)

**Interfaces:**
- Consumes: `layout::layout(title, audio, subtitles) -> Layout`, `default_subtitle`, the test helpers of `layout.rs` (`film_title`, the audio and subtitle builders, the defaults collector used by the test at `:587`).
- Produces: nothing new in code.

- [ ] **Step 1: Write the failing layout-level test**

Beside `with_non_english_default_audio_the_regular_subtitle_is_flagged_default_and_the_forced_is_not`, built the same way but with only a Forced track (stream 5) and an SDH track (stream 6) kept:

```rust
#[test]
fn with_non_english_default_audio_and_no_regular_track_the_sdh_subtitle_is_flagged_default() {
    // OR3: the SDH track is default when it is the only English dialogue track kept.
    /* same construction as the test above, kept subtitles [Forced 5, Sdh 6] */
    assert_eq!(defaults, vec![(5, false), (6, true)]);
}
```

- [ ] **Step 2: Run it to verify it passes (it pins existing behaviour)**

Run: `scripts/box.sh cargo test -p lazymkv-core tracks::layout`
Expected: PASS. (A pin of confirmed behaviour; the red step is skipped by design.)

- [ ] **Step 3: Edit the documents**

Design spec, one sentence each:
- `:36` (D9): "recorded as a note on the job" becomes "recorded as the note `lossy_before_lossless` on the job, naming the lossy track (OR8)".
- `:45` (D15), last sentence becomes: "Until then a track with the commentary pattern is kept; a same-language commentary verdict sends the job to review with `commentary_detected` (the port's ruling RP7, 2026-10-14)."
- `:197` and `:201`: "a note records the switch" becomes "the note `direct_mode_fallback` records the switch".
- `:226`: "to be designed in phase 1" becomes "to be designed in phase 2 (the engine design)".
- `:349-353` (8.6 step 6), two new bullets after the `lossless_original_available` one: "- the content rule's ad-pattern or note-ad verdict: note `possible_audio_description`, naming the track (RP7);" and "- a lossy track with more channels placed before a lossless track with fewer (D9): note `lossy_before_lossless`, naming the lossy track;".
- `:355`: "Whether it also sends the job to review (`commentary_detected`, row 13's fallback) is the owner's decision, still open under D15." becomes "A same-language commentary verdict sends the job to review with `commentary_detected` (RP7)."
- `:363`: "the remedy to try first in phase 1" becomes "the remedy to try first in phase 2 (the engine design)".
- `:397-407` (8.8 table): a row "| `commentary_detected` | 8.6 (the content rule's commentary verdict, D15, RP7) |" after the `audio_reference_unclear` row.
- `:443` (8.9): "trailing spaces and periods trimmed" becomes "leading and trailing spaces and periods trimmed (a component beginning with a period would be hidden on the share: ruling OR4)".
- `:447` (8.10), after "the regular English track is default and the forced track is not": "; when no regular English track is kept, the SDH track is default (ruling OR3)".
- `:571` (10.2): "a note records why" becomes "the note `label_rule_unavailable` records why (not on a DVD, which carries no labels)".
- `:700` (phase 2 row), appended before the closing `|`: ". The phase 2 design also settles D17's bundling table, the app's own recording format, the form of the tuning evidence, the decoding of bitmap subtitles for OCR (D22) and code signing (RP10, OR11)".
- `:743`: "(phase 1 design; D17)" becomes "(phase 2 design; D17)".

`layout.rs:105-106`: `and is a choice the owner may overturn` becomes `the owner's confirmed choice (ruling OR3, 2026-10-17)`. `naming.rs:59-60`: `a choice beyond 8.9's list` becomes `the owner's confirmed addition to 8.9's list (ruling OR4)`.

`decision-core.md`: `:271-275` (RP25) gains "(confirmed by ruling OR9, 2026-10-17)" at its end; `:284-286` (RP27) becomes `- RP27. The four note names the design spec described only in words are confirmed (ruling OR8, 2026-10-17) and the spec now names them: ...`; the three open-question bullets `:371-381` become `- Ruling OR2 (2026-10-17): two cuts named alike ... keep 0.60 with title_ambiguous.`, `- Ruling OR3: ... the SDH track is the default subtitle.`, `- Ruling OR4: ... leading periods and spaces are dropped.`; `:398-399` gains `(ruling OR7: kept provisional)`.

- [ ] **Step 4: Check the spec reads consistently**

Run: `grep -n "phase 1" docs/superpowers/specs/2026-10-04-lazymkv-design.md`
Expected: only lines describing finished work (323, 680, 688, 690).

- [ ] **Step 5: Commit**

```bash
git add docs crates/core/src/tracks/layout.rs crates/core/src/naming.rs
git commit -m "spec: the owner's rulings OR2 to OR4, OR7, OR8 and OR11 written into the design; the four note names named"
```

---

### Task 5: Core test gaps A: lang, codec, settings, disc

**Files:**
- Modify: `crates/core/src/plan/movie.rs:12` (import), `:401-409` (`coverage`), tests
- Modify: `crates/core/src/codec.rs:231-245` (`dts_family`), docs `:150-151`, tests beside `:527`
- Modify: `crates/core/src/settings.rs:254-260` (doc), tests beside `:778`
- Modify: `crates/core/src/disc.rs:497-502`, `:562-575` (`character_reference`), tests `:1256-1273`, beside `:1340`

**Interfaces:**
- Consumes: `lang::is_english(code) -> bool`; `codec::codec(id, short, long) -> Codec`, `Family`, the `known(family, display, object, lossless)` test helper; `Settings::validate`, the `problem_of` helper; `disc::character_reference(tail) -> Option<(char, usize)>`, `bdmt_name(xml) -> Option<String>`.
- Produces: `fn dts_named(text: &str) -> Option<Family>` (private; `DTS-HD MA`/`DTS-HD Master Audio` MA, `DTS-HD HR`/`DTS-HD High Resolution Audio` HR, `DTS-HD LBR`/`DTS Express` Express, `DTS` Dts).

- [ ] **Step 1: Write the failing tests**

`movie.rs` (SR4), with a local `const EN_AC3: &[(u32, &str)] = &[(1, "Audio"), (3, "en"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];`:

```rust
#[test]
fn a_two_letter_english_code_counts_as_english_when_a_cut_chooses_its_variant() {
    let disc = java_disc(&[(1, "FPL_MainFeature", &[JPN_AC3]), (2, "FPL_MainFeature_SPA", &[EN_AC3])]);
    let plan = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
    assert_eq!(plan.features, vec![feature(2, Some("Theatrical Cut"), true, &[1])]);
}
```

`codec.rs` (SR5):

```rust
#[test]
fn a_dts_stream_named_by_one_attribute_only_reads_by_that_attribute() {
    assert_eq!(codec("A_DTS", "", "DTS-HD Master Audio"), known(Family::DtsHdMa, "DTS-HD MA", None, true));
    assert_eq!(codec("A_DTS", "", "DTS"), known(Family::Dts, "DTS", None, false));
    assert_eq!(codec("A_DTS", "DTS-HD MA", ""), known(Family::DtsHdMa, "DTS-HD MA", None, true));
    assert_eq!(codec("A_DTS", "DTS-HD HR", ""), known(Family::DtsHdHr, "DTS-HD HR", None, false));
    assert_eq!(codec("A_DTS", "DTS", ""), known(Family::Dts, "DTS", None, false));
}

#[test]
fn a_conflicting_dts_pair_is_read_by_attribute_6_then_7() {
    assert_eq!(codec("A_DTS", "DTS", "DTS-HD Master Audio").family, Family::Dts);
    assert_eq!(codec("A_DTS", "DTS-HD HR", "DTS-HD Master Audio").family, Family::DtsHdHr);
    assert_eq!(codec("A_DTS", "DTS-HD LBR", "DTS-HD Master Audio").family, Family::DtsExpress);
    assert_eq!(codec("A_DTS", "DTS-HD MA", "DTS").family, Family::DtsHdMa);
    assert_eq!(codec("A_DTS", "DTS", "DTS:X").family, Family::DtsHdMa, "DTS:X in either attribute wins");
}
```

`settings.rs`: `a_tuning_root_inside_the_staging_root_is_refused_even_with_keep_tuning_evidence_off`, a copy of `a_tuning_root_inside_the_staging_root_is_refused` with `keep_tuning_evidence: false` and the same expected problem.

`disc.rs` (SR6):

```rust
#[test]
fn character_reference_refuses_surrogates_overflow_a_missing_semicolon_and_empty_digits() {
    for tail in ["&#xD800;", "&#55296;", "&#x110000;", "&#4294967296;", "&#233", "&#233 more; text", "&#;", "&#x;", "&#X41;"] {
        assert_eq!(character_reference(tail), None, "{tail}");
    }
    assert_eq!(character_reference("&#65;"), Some(('A', 5)));
    assert_eq!(character_reference("&#x41;"), Some(('A', 6)));
    assert_eq!(character_reference("&#x10FFFF;"), Some(('\u{10FFFF}', 10)));
}

#[test]
fn bdmt_name_leaves_a_refused_reference_as_written() {
    assert_eq!(bdmt_name("<di:name>A &#xD800; B &#X41; C&#;</di:name>").as_deref(), Some("A &#xD800; B &#X41; C&#;"));
}

#[test]
fn bdmt_name_does_not_decode_a_character_xml_forbids() {
    // Review Focus 4 and SR6: XML 1.0's Char production excludes U+0000, U+0001 to U+0008, U+000B, U+000C, U+000E to
    // U+001F, U+FFFE and U+FFFF; such a reference stays as written, like a refused entity.
    for (xml, name) in [("A&#0;B", "A&#0;B"), ("A&#x1F;B", "A&#x1F;B"), ("A&#xFFFE;B", "A&#xFFFE;B"), ("A&#9;B", "A\tB")] {
        assert_eq!(bdmt_name(&format!("<di:name>{xml}</di:name>")).as_deref(), Some(name), "{xml}");
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core plan::movie::tests::a_two codec:: settings:: disc::`
Expected: the `coverage`, conflicting-pair and XML tests FAIL; the others PASS (pins).

- [ ] **Step 3: Implement**

- `movie.rs:406`: `let english = languages.iter().any(|code| is_english(code));` with the import.
- `codec.rs`: `dts_named`; `dts_family` = the DTS:X check, then `dts_named(short).or_else(|| dts_named(long)).unwrap_or(Family::Unknown)`; docs at `:150-151` and `:231-233` already say attribute 6 then 7.
- `settings.rs:254-258` doc: `a set tuning_root inside staging_root or a library root, whatever keep_tuning_evidence says`.
- `disc.rs` `character_reference`: after `char::from_u32`, return `None` unless `matches!(character, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')`; docs at `:497-502` and `:562-563`. Test helpers `feature` and `dvd_title` end in the single tail expression `typed.title(number).unwrap().clone()`.

- [ ] **Step 4: Run the core tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/plan/movie.rs crates/core/src/codec.rs crates/core/src/settings.rs crates/core/src/disc.rs
git commit -m "core: pin the lang, codec, settings and disc test gaps; coverage uses is_english, DTS reads attribute 6 first, XML-forbidden references stay as written"
```

---

### Task 6: Core test gaps B: the planner

**Files:**
- Modify: `crates/core/src/lookup/tests.rs` (after `:1004`)
- Modify: `crates/core/src/plan/movie.rs` tests (after `:1040`)
- Modify: `crates/core/src/plan/mod.rs:945-952` (fixtures), tests (`:1576-1603`, `:1712`, `:1607`, `:2129`, new)
- Modify: `crates/core/src/plan/tv.rs:521-522` (fixture), tests
- Modify: `crates/core/src/remembered.rs:613-619` (fixtures), its uses
- Modify: `crates/core/src/tracks/audio/tests.rs`

**Interfaces:**
- Consumes: `lookup::identify(inputs, kind, runtime, settings) -> Identity` and the `tests.rs` helpers (`label`, `search`, `movie_record`, `movie`, `features`, `episode_groups`); `movie::by_labels`; `plan::plan` with the `mod tests` helpers (`made_disc`, `made`, `orphanage_record`, `searched`, `inputs`, `run`, `planned`, `feature`, `cut`, `episodes_disc`, `show_search`, `show_record`, `assigned_episode`); `plan::mode_sentence(title_confidence, planned) -> String` (private); `plan::assigned_feature(number, role) -> Option<Feature>`; `tv::without_play_all(groups, dvd) -> (Vec<Group>, Vec<u32>)` and `tv.rs`'s `group(title, segments, seconds)`; `tracks::audio::decide_audio(title, wanted, facts, settings)` and the `audio/tests.rs` helpers (`title_of`, `video`, `truehd`, `dts_hd_ma`, `with`, `audio`, `order_of`, `names`).
- Produces: `plan::fixtures::ENG_AC3_2CH` (`(1,"Audio"),(3,"eng"),(5,"A_AC3"),(6,"DD"),(7,"Dolby Digital"),(14,"2")`); `remembered.rs` and `tv.rs` use it and `ENG_AC3` instead of their rows; a `dts_x(language, channels)` helper in `audio/tests.rs` beside `dts_hd_ma` (attribute 7 `DTS-HD Master Audio DTS:X`).

- [ ] **Step 1: Write the tests (all pins of existing behaviour; SR7)**

`lookup/tests.rs`:

```rust
#[test]
fn the_runtime_check_of_a_movie_result_reads_the_features_even_when_the_structure_says_tv() {
    let name = label("EXAMPLE_FILM");
    let results = search(&[("movie", 1001, "Example Film", "Example Film", "1986-05-09")]);
    let record = movie_record(1001, "Example Film", "1986-05-09", Some(100));
    let inputs = LookupInputs { name: name.as_ref(), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let held = identify(&inputs, Kind::Tv, &episode_groups(&[6000]), &Settings::default());
    assert_eq!((held.step, held.confidence, held.tmdb, held.runtime_check, held.review),
        (IdentityStep::TmdbSeveral, 0.50, Some(movie(1001)), Some(false), Some(ReviewReason::IdentityAmbiguous)));
    assert_eq!(held.evidence.last().map(String::as_str), Some("the runtime check fails"));
    let found = identify(&inputs, Kind::Tv, &features(&[6000]), &Settings::default());
    assert_eq!((found.step, found.confidence, found.runtime_check), (IdentityStep::TmdbOne, 0.90, Some(true)));
}
```
(Build `LookupInputs` as the neighbouring test `the_runtime_check_is_the_one_of_the_results_own_kind` does if its shape differs.)

`movie.rs` (T5): `a_lone_cut_with_an_other_suffix_is_unnamed_at_0_60`: `by_labels(&java_disc(&[(1, "FPL_MainFeature_IMAX", &[ENG_AC3])]), &wanted_languages(None))` equals `Some(MoviePlan { rule: TitleRule::Labels, features: vec![feature(1, None, false, &[])], feature_seconds: vec![Some(6000)], confidence: 0.60, reviews: vec![ReviewReason::UnnamedCut], notes: vec![], rivals: vec![], evidence: strings(&["label cut FPL_MainFeature_IMAX: titles [1], title 1 chosen"]) })`. In `plan/mod.rs:2129`'s test add `assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));`.

`tv.rs` (T6):

```rust
#[test]
fn without_play_all_returns_the_groups_that_remain_and_the_dropped_titles() {
    let groups = vec![group(0, &[12], 1320), group(1, &[13], 1320), group(4, &[10, 11], 600), group(5, &[12, 13], 2640), group(6, &[10], 300), group(7, &[11], 300)];
    assert_eq!(without_play_all(&groups, false), (vec![group(0, &[12], 1320), group(1, &[13], 1320), group(6, &[10], 300), group(7, &[11], 300)], vec![4, 5]));
    assert_eq!(without_play_all(&groups, true), (groups.clone(), vec![]), "a DVD never drops");
    let plain = vec![group(0, &[1], 1320), group(1, &[2], 1320)];
    assert_eq!(without_play_all(&plain, false), (plain.clone(), vec![]));
}
```

`plan/mod.rs` (T7):
- In `the_label_variant_is_rechosen_once_the_original_language_is_known`: for the Spanish plan `assert!(plan.evidence.sentences.contains(&"8.3 rule 2 chose each cut's variant again with the original language (spa)".to_string()));`, for the English plan `assert!(!plan.evidence.sentences.iter().any(|s| s.starts_with("8.3 rule 2 chose")));`.
- `a_second_label_pass_keeps_every_cut_and_changes_only_the_variant`: `made_disc(true, "THE_ORPHANAGE", ...)` with titles 1 `FPL_MainFeature` ENG, 2 `FPL_MainFeature_SPA` ENG_SPA, 3 `FPL_MainFeature_UR` ENG, 4 `FPL_MainFeature_UR_SPA` ENG_SPA (durations 1:40:00, chapters 16, playlists `0000N.mpls`, segments `N`, sizes 30 to 33 GB); with the English record the planned titles are `[1, 3]` with roles `feature(Some("Theatrical Cut"))`, `cut(Some("Unrated Cut"))`; with `orphanage_record("es", Some(100))` they are `[2, 4]` with the same roles, `title_rule` `Labels`, `(0.90, Mode::Direct)`, and the sentence above present.
- `mode_sentences_name_the_confidence_and_the_mode`: `mode_sentence(0.90, Mode::Direct) == "title confidence 0.90: Direct mode (6.2)"`, `mode_sentence(0.6, Mode::Copy) == "title confidence 0.60: whole-disc copy mode (6.3)"`; and in `an_unnamed_cut_holds_the_plan_at_0_60_in_copy_mode` and `with_tmdb_only_autofile_off_a_search_identity_is_capped_at_0_84_and_held` assert `plan.evidence.sentences.last()` is that sentence for their confidence and mode.
- `assigned_episodes_join_the_season_session_even_under_an_unconfirmed_search_identity` (SR7): `episodes_disc("SHOW", false)`, `searched(name, &show_search(), Some(&show_record(2)))`, overrides assigning titles 1, 2, 3 to `TitleRole::Episodes { season: 2, episodes: vec![n] }`, fingerprint `"FP"`: `(identity.step, identity_confidence) == (IdentityStep::TmdbOne, 0.90)`, kind `Tv`, every title `remembered`, and `plan.session == Some(SeasonSession { show_id: 99, season: 2, discs: vec!["FP".into()], assigned: vec![assigned_episode(1, "FP", 1), assigned_episode(2, "FP", 2), assigned_episode(3, "FP", 3)], episode_count: Some(6), closed: false })`; the same overrides with no search and no record give `session == None`. (Field names and the `99`/`6` values as `show_search`/`show_record` define them; the implementer reads those helpers.)
- `an_assigned_cut_named_theatrical_version_is_flagged_theatrical`: `assigned_feature(1, &TitleRole::Cut { edition: "Theatrical Version".into() }) == Some(Feature { title: 1, edition: Some("Theatrical Version".into()), theatrical: true, siblings: vec![] })`.

`audio/tests.rs` (T8, SR7):
- `an_ffprobe_extension_wins_over_attribute_7_for_the_object_and_the_name`: `title_of(&[video(), truehd("eng", "8"), truehd("eng", "8")])` with `ffprobe_profile` `{1: "TrueHD", 2: "Dolby TrueHD + Dolby Atmos"}` through `decide_audio(.., &wanted_languages(Some("en")), &facts, &Settings::default())`: `order_of == [2, 1]`, `names == ["TrueHD Atmos 7.1", "TrueHD 7.1"]`, the first kept track's `object == Some(ObjectAudio::Atmos)`; and a stream whose attribute 7 says `TrueHD Atmos` with ffprobe `DTS:X` gives `object == Some(ObjectAudio::DtsX)` and the name `TrueHD DTS:X 7.1`, while at planning time (default facts) it is `Atmos`.
- `a_dts_x_track_is_named_after_the_dts_hd_ma_core_its_extension_and_its_layout`: `title_of(&[video(), dts_hd_ma("eng", "8"), dts_x("eng", "8")])`: at planning time `order_of == [1, 2]`, `names == ["DTS-HD MA 7.1", "DTS-HD MA DTS:X 7.1"]`, `kept[1].object == Some(ObjectAudio::DtsX)`; with `ffprobe_profile` `{1: "DTS-HD MA", 2: "DTS-HD MA + DTS:X"}` `order_of == [2, 1]`; a 6-channel DTS:X stream is named `DTS-HD MA DTS:X 5.1`.

Fixtures (T6): add `ENG_AC3_2CH` to `plan::fixtures`; delete `remembered.rs`'s `ENG_AC3_6CH` and `ENG_AC3_2CH` (use `ENG_AC3` and the fixture) and `tv.rs`'s `ENGLISH_STEREO`; `decision.rs` and `crates/dev/src/cmd/plan.rs` keep their rows (they differ or cannot see core's fixtures).

- [ ] **Step 2: Run them**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: PASS. If a pin of SR7 behaviour fails, the report says which value the code gives; the controller rules (the pin follows the code unless the code contradicts the spec).

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/lookup/tests.rs crates/core/src/plan crates/core/src/remembered.rs crates/core/src/tracks/audio/tests.rs
git commit -m "core::plan: pin the planner's test gaps (runtime check by result kind, lone unnamed cut, the second label pass, mode sentence, assigned episodes' session, object-audio names); one stereo fixture"
```

---

### Task 7: Core test gaps C: subtitles, layout, verify, decision, job

**Files:**
- Modify: `crates/core/src/tracks/subtitle/tests.rs` (after `:274`, after `:383`, in `mod roles` after `:2186`)
- Modify: `crates/core/src/tracks/layout.rs` tests `:530-550`
- Modify: `crates/core/src/verify.rs` tests (after `:1944`)
- Modify: `crates/core/src/decision.rs` tests (after `:1326`, `:1910`)
- Modify: `crates/core/src/job/tests.rs:92-180`

**Interfaces:**
- Consumes: `subtitle::{parent_of, set_aside, decide_subtitles, SubtitleFacts, SubtitleOverride, SubtitleOverrideEntry, signature}`, the `tests.rs` helpers (`title_of(dvd, &[(kind, language, flags)])`, `bluray`, `facts`, `regular_events`, `sdh_events`, `verdict`, `track`); `layout::language_tag` through `layout()` and `audio_track(..)`; `verify::subtitle_events(tree, subs_dir, report, clear_max_bytes)` with `subs_tree`, `INDEX`, `IMAGE_PACKETS`, `dvd_report`; `decision::decide` with `film_disc`, `office_disc`, `plan_of`, `feature_role`, `episode_role`, `film_record`, `office_record`, `settings`, `inputs`; `job::Event`, `remembered::{Overrides, TitleOverrideEntry, TitleRole, AudioTrackOverride, AudioOverride, SubtitleTrackOverride, SubtitleOverride, CollisionAnswer, Collision}`, `lookup::tmdb::{TmdbIdentity, TmdbKind}`, `tracks::SubtitleRole`.
- Produces: nothing new in code.

- [ ] **Step 1: Write the tests (pins; expected values derived by the readers from the code, confirmed by the run)**

`subtitle/tests.rs`:
- T9a `two_different_no_language_codes_are_one_language_for_a_forced_only_parent`: `title_of(false, &[("video","",0), ("pgs","zxx",0), ("pgs","und",6144)])`: `parent_of(&subtitles, 2) == Some(1)`, `set_aside(&t).forced_only == vec![(2, Some(1))]`, `counted == vec![1]`, `other_language` empty; the reverse order (`und` parent, `zxx` child) and the pairs `("mul","mis")`, `("-","und")`, `("","zxx")` likewise.
- T9b `a_dvd_closed_caption_stream_flagged_directors_comments_is_set_aside_as_directors_comments`: `title_of(true, &[("video","",0), ("vobsub","eng",0), ("cc","eng",1), ("cc","eng",0)])`: `directors_comments == [2]`, `closed_captions == Some(3)`, `counted == [1]`; flags `2` likewise. `a_derived_forced_only_closed_caption_stream_is_set_aside_as_forced_only`: `title_of(true, &[("video","",0), ("vobsub","eng",0), ("cc","eng",6144)])`: `forced_only == [(2, Some(1))]`, `closed_captions == None`, `counted == [1]`.
- T10 `a_stream_the_owner_decided_is_not_compared_with_the_text` (in `mod roles`): title `bluray(&[VIDEO, ENG_PGS, ENG_PGS, SPA_PGS])`, `facts(vec![(1, regular_events()), (2, sdh_events())])`, `text = BTreeMap::from([(1, verdict(Role::Sdh))])`, `reviewed = BTreeMap::from([(1, SubtitleOverride::Keep { role: SubtitleRole::Regular })])`: `reviews == []`, `notes == []`, confidence `0.85`, kept `[track(1, Regular, Some(1000), "reviewed"), track(2, Sdh, Some(1200), "alignment")]` (the event counts as the helpers give them); a twin with the same override given through `remembered: Some(&[SubtitleOverrideEntry { signature: signature(&title.streams, 1), action: SubtitleOverride::Keep { role: SubtitleRole::Regular } }])`.

`layout.rs`: extend `a_no_language_audio_track_has_no_language_tag` with `audio_track(7, Some("-"), ..)`, `audio_track(8, Some("UND"), ..)`, `audio_track(9, Some("Und"), ..)`, `audio_track(10, Some("mis"), ..)`, `audio_track(11, Some("ZXX"), ..)` expecting `None, None, None, Some("mis"), Some("ZXX")` (the rest of each row as the existing rows).

`verify.rs`:
- `subtitle_events_prefix_a_packets_file_that_is_not_an_object_with_its_path`: `subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", "[]"), ("subs/sub-1.packets.json", IMAGE_PACKETS)])` gives `Err("rec/subs/sub-0.packets.json: 'list' object has no attribute 'get'")`.
- `subtitle_events_never_prefix_a_path_twice`: packets `{` and `{"packets": 5}`: the error starts with `rec/subs/sub-0.packets.json: ` and contains the path once.
- `subtitle_events_prefix_an_events_error_with_the_path`: `{"packets": [{"pts_time": "1.0", "size": "big"}]}` gives `Err("rec/subs/sub-0.packets.json: invalid literal for int() with base 10: 'big'")`; `{"packets": [{"pts_time": "soon", "size": "20"}]}` gives `...: could not convert string to float: 'soon'`; a packet with a bad size and no `pts_time` is skipped without error.

`decision.rs`:
- T13a `a_feature_without_a_video_height_builds_no_path_and_is_held_without_a_review_reason`: `film_disc()` with title 0's video stream's size cleared (`video_size = None` on `streams[0]`, or a fixture without attribute 19), `plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))])`, `film_record(12345, "Film", 2010, Some("tt0012345"))`, `settings()`: `path == None`, `resolution == None`, `reviews` empty, `disposition == Disposition::NeedsReview`; the Episode counterpart on `office_disc()` still has a path with `resolution == None`.
- T13b `an_episode_with_a_season_but_no_episode_numbers_builds_no_path_and_raises_tv_unmapped`: `episode_role(Some(3), &[], Some("Example Fifteen"))` on `office_disc()`/`office_record()`: `path == None`, `reviews == vec![ReviewReason::TvUnmapped]`, `NeedsReview`; with `Overrides { titles: vec![TitleOverrideEntry { title: 0, role: TitleRole::Episodes { season: 3, episodes: vec![] } }], .. }` still `TvUnmapped`, no path, `confidences.title == 1.00`.
- T13c `a_tmdb_record_given_without_an_identity_is_ignored`: `plan_of(&disc, Kind::Movie, None, &[(2, feature_role(None))])` over `film_disc()`, a `film_record` with `original_language = Some("es".into())`: `decide` is `Ok`, `path == None`, `NeedsReview`, and `audio.dropped.contains(&(2, AudioDrop::UnwantedLanguage))` (the plan's English-only wanted set stands).

`job/tests.rs` (T14): a third `ReviewApproved` sample with `identity: Some(TmdbIdentity { kind: TmdbKind::Tv, id: 2316 })`, `season: Some(3)`, titles `[Episodes { season: 3, episodes: vec![12, 13] }, Cut { edition: "Extended Cut".into() }, Ignored]`, audio `[Keep { order: 0, default: true }, Commentary]`, subtitles `[Keep { role: SubtitleRole::Forced }, Drop]`, collision one `CollisionAnswer` per `Collision` value; the doc comment's event count updated; and an exact-JSON row for one `ReviewApproved` in `an_event_is_written_with_its_name_under_event_and_its_fields_beside_it`.

- [ ] **Step 2: Run them**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: PASS. A pin whose derived value differs from the run is corrected to the code's value and the difference reported (the controller rules whether the code is wrong).

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/tracks crates/core/src/verify.rs crates/core/src/decision.rs crates/core/src/job/tests.rs
git commit -m "core: pin the subtitle, layout, verify, decision and job test gaps"
```

---

### Task 8: Dev test gaps

**Files:**
- Modify: `crates/dev/src/cmd/plan.rs:216`, tests (after `:948`)
- Modify: `crates/core/src/subtitle/labels/row14.rs:254-255`, `crates/core/src/subtitle/labels.rs:374` (re-export), `row14/tests.rs` (after `:1081`)
- Modify: `crates/dev/src/cmd/retune.rs:45-46`, `cmd/decide.rs:90-91`, `cmd/golden.rs:367`, `:374`, `crates/dev/tests/decide.rs:52`
- Modify: `crates/dev/tests/labels_subtitles.rs`

**Interfaces:**
- Consumes: `plan_doc(ctx, source, options) -> Result<PlanDoc, Refused>`, `PlanOptions { tmdb_record, .. }`, the `plan.rs` test helpers (`film_recording`, `planned`, `found`, `Scratch`, `tmdb_movie_json`); `row14::harness` and the `row14/tests.rs` helpers (`judge`, `trees`, `bluray`, `spec_row14`); `labels_subtitles.rs`'s `call`, `scratch_with_disc_b`, `read_json`, `ONE_LINE_FILE`.
- Produces: `pub const TEXT_HINT: &str = "(give --text DIR or LAZYMKV_OCR_TEXT)"` in `core::subtitle::labels` (SR9), used by `retune.rs`, `decide.rs`, `golden.rs` and `crates/dev/tests/decide.rs`; `plan.rs:216` `tmdb_pending: found.tmdb_pending && given_record.is_none()` (SR8).

- [ ] **Step 1: Write the failing test for SR8 in `plan.rs`**

```rust
#[test]
fn a_tmdb_record_file_clears_the_pending_flag_of_a_failed_tmdb_lookup() {
    let tmp = Scratch::new("plan-record-clears-pending");
    film_recording(&tmp, "rec");
    tmp.write("rec/lookup-status.txt", "dischash 200\ndiscid 404\ntmdb movie/6537 500\n");
    tmp.write("movie.json", &tmdb_movie_json(6537, "Made Up Film", "Pelicula Inventada", "2007-05-20", "fr", Some(100)));
    let pending = planned(&tmp, &PlanOptions::default());
    assert!(pending.plan.identity.lookup_pending, "the regression guard: without the option the lookup is pending");
    let doc = planned(&tmp, &PlanOptions { tmdb_record: Some(PathBuf::from("movie.json")), ..PlanOptions::default() });
    assert_eq!(doc.inputs.tmdb, found("rec/tmdb.json", "failed"));
    assert!(!doc.plan.identity.lookup_pending);
    assert!(!doc.plan.evidence.sentences.iter().any(|s| s == "the TMDb lookup is pending"), "{:?}", doc.plan.evidence.sentences);
    assert_eq!(doc.plan.identity.original_language.as_deref(), Some("fr"));
    // Review Focus 5: a failed TheDiscDB lookup keeps the job pending whatever record is given.
    tmp.write("rec/lookup-status.txt", "dischash 500\ndiscid 404\ntmdb movie/6537 500\n");
    let doc = planned(&tmp, &PlanOptions { tmdb_record: Some(PathBuf::from("movie.json")), ..PlanOptions::default() });
    assert!(doc.plan.identity.lookup_pending);
}
```
(The exact `film_recording` status lines and `found` shape as the neighbouring test `a_failed_lookup_is_planned_as_pending` uses them.)

- [ ] **Step 2: Run it to verify it fails**

Run: `scripts/box.sh cargo test -p lazymkv-dev a_tmdb_record_file_clears`
Expected: FAIL on `lookup_pending`.

- [ ] **Step 3: Fix `plan.rs:216` and write the remaining tests**

- `tmdb_pending: found.tmdb_pending && given_record.is_none()`, with a doc sentence (SR8: a given record replaces the lookup's; `--tmdb-search` does not clear the flag).
- T17 in `labels_subtitles.rs`: `set_warns_about_the_problems_the_file_already_had`: write `disc-b/subtitle-labels.json` as `ONE_LINE_FILE` plus a second track `all-tracks` stream 9 with `"sure": "yes"`; `call(.., &["--subtitles", "set", "disc-b", "all-tracks", "4", "regular", "--how", "read", "--saw-verdict", "no", "--date", "2026-10-15"])` exits 0 with empty stderr and stdout ending `warning: the file still has 1 problem(s): track all-tracks stream 9: sure must be true or false\n`; `unset_warns_about_the_problems_the_file_already_had` likewise for `unset disc-b all-tracks 4 --note x`; a file with two old problems prints `2 problem(s)` joined by `; `; an unchanged `set` prints the warning and does not rewrite the file; after the edit `check` still exits 2.
- T18a `the_text_rule_reads_the_settings_when_ocr_text_is_given` in `row14/tests.rs`: the fixtures of `subs_tolerance_ms_decides_whether_two_tracks_align` with both streams read, judged with `Some(&text)` at `Settings::default()` and at `Settings { subs_tolerance_ms: 600, ..Settings::default() }`; assert the two runs' `text ...` words of the Tracks lines differ, and pin each to the value the first mini PC run shows (recorded in the test with a comment saying so); the same for `sub_clear_max_bytes: 29` on the read `disc_a`.
- T18b: `pub const TEXT_HINT` in `row14.rs`, re-exported from `labels.rs`; `retune.rs`, `decide.rs`, `golden.rs` and `crates/dev/tests/decide.rs` use `lazymkv_core::subtitle::labels::TEXT_HINT` and drop their copies.

- [ ] **Step 4: Run the workspace tests with the clone**

Run: the junction form of `scripts/box.sh`
Expected: PASS (`retune_subtitles.rs`, `golden.rs` and `decide.rs` keep their literal expectations).

- [ ] **Step 5: Commit**

```bash
git add crates/dev crates/core/src/subtitle
git commit -m "dev: a given TMDb record clears the pending flag; one TEXT_HINT; pin the labels warning and the text rule's settings"
```

---

### Task 9: Closing documents and the full run

**Files:**
- Modify: `docs/phase1/decision-core.md:312-313` (intro of the review block), after `:358` (new block), `:360-364` (intro of "Choices"), `:439-445` (test count)
- Modify: `README.md:22-23`

**Interfaces:**
- Consumes: the rulings OR1 to OR16 and SR1 to SR9 of this plan; the mini PC's `cargo test --workspace -- --list` counts.
- Produces: nothing in code.

- [ ] **Step 1: Run the full suite with the clone and list the tests**

Run: the junction form of `scripts/box.sh` with `cargo test --workspace --locked`, then the same with `cargo test --workspace -- --list` and count per crate (`grep -c ": test$"` per binary group as the earlier counts were taken).
Expected: 0 failed; counts above 1986 in total.

- [ ] **Step 2: Write the rulings block and the counts**

- After F15 (`:358`), a block `### The owner's rulings of 2026-10-17` opening `After the merge the owner ruled on the open questions and the follow-ups (OR1 to OR16); the controller's sub-rulings SR1 to SR9 settled what the readers found while planning. The owner may overturn any of them.` followed by one bullet per ruling in the words of this plan's Rulings section.
- `:360-364`: the intro of "Choices the owner should confirm" becomes `The plan settled these where the spec leaves a choice open; each is pinned by a test, and each can be changed by changing that test and its rule. The four open questions were ruled on 2026-10-17 (OR1 to OR4 above).`
- `:439-445`: a sentence `After the rulings pull request the same command lists N: a in lazymkv-core, b in lazymkv-dev, 8 in lazymkv-capture and 3 in fake-tools.` with the counts of Step 1.
- `README.md:22-23`: `what the owner may still overturn` becomes `the owner's rulings and what may still be overturned`.

- [ ] **Step 3: Commit**

```bash
git add docs/phase1/decision-core.md README.md
git commit -m "docs: the owner's rulings of 2026-10-17 and the test count after the rulings pull request"
```
