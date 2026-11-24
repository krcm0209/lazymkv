# Engine menu maps, pull request 1: the rule and the map's files, implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `lazymkv-dev decide --menu` decides by the rules the owner ruled on 2026-11-24, shows a note for every
track the menu drops, reads off-decisions that are bound to their map, and a map can be listed and published from the
Mac.

**Architecture:** Three changes in `core` (the off file's third field, two new keys of the `roles` block, the rule in
`decide_audio` with its notes and outcomes) and two in `dev` (the wiring that hands the rule its new inputs, and the
`menu off` and `menu publish` commands). The engine is not touched: its pull request is planned after phase 3's
fix-after is merged (the owner's order, EO-16).

**Tech Stack:** Rust 1.100.0, the workspace's crates only. No new crate.

**Spec:** `docs/superpowers/specs/2026-11-24-engine-menu-maps-design.md`, parts 8, 9, 12, 13 and 16 (its pull request
1 of part 16.1). The rule as built so far: `docs/menu-maps/format.md`, "The rule".

## Rulings of this plan (the owner can correct them)

- EP-1: the `roles` block's keys are, in order, `acts`, `why`, `audio`, `unplaced`, `off_unused`, `off_stale`, `play`.
  The two new ones are always written (`[]`, `null`) and are read as empty and null when a document lacks them.
- EP-2: the off listing of spec part 9 is `lazymkv-dev menu off <map>`.
- EP-3: the "one public function" of spec part 10.3 is two calls, `read_menu_pair` then `menu_input`, so that
  `decide`'s order of refusals (the map and its off file before the recording) stays as it is.
- EP-4: the size limits and the disc-folder check live in `core::menu` (`MAP_MAX_BYTES`, `OFF_MAX_BYTES`,
  `disc_folder`), so the engine's later pull request reads maps by the same three.
- EP-5: this pull request adds the two notes of a decision (`menu_dropped`, `menu_reference_conflict`). The job's own
  note `menu_map_not_used`, the setting `menu_maps_root` and the job summary's notes belong to the engine's pull
  request.
- EP-6: the notes' sentences are fixed in Task 3, within the spec's examples.
- EP-7: `would_leave_none` stays as a guard. Through `decide --menu` it can no longer be reached: with a reference the
  reference stays, and without one nothing is dropped unless Play's own stream is kept, which is never a candidate
  (MR-17). Its test hands `decide_audio` a confirmation the wiring never gives.
- EP-8: the title-level outcomes are tested in this order: `reference_disputed`, `no_reference`, `would_leave_none`,
  `reference_would_change`, else `dropped`.
- EP-9: the acceptance scripts read the owner's private data and are not committed. The repository gets counts only.

## Global Constraints

- Audio only. Without `--menu` every document of every command is byte-identical to `main`.
- The guards stay exactly as built: MR-3, MR-4, MR-5, MR-6, MR-8, MR-9, MR-13, MR-17, MA-2 and "never the programme
  reference".
- `crates/engine`, `crates/ctl`, `crates/core/src/verify.rs` and `crates/core/src/codec.rs` are not touched.
- Tests use made-up maps, playlists, scans and recordings only. No disc title, person's name, host, share or user
  name in any committed file. Nothing under `~/lazymkv-menu-spike/` is copied into the repository.
- `Cargo.lock` does not change. No warning.
- Builds and tests run on the Mac. `$CARGO` below stands for
  `env RUSTUP_AUTO_INSTALL=0 PATH="$HOME/.cargo/bin:$PATH" LAZYMKV_OCRS_MODELS="$HOME/.cargo/lazymkv-ocrs-models" cargo`.
  The full run is `$CARGO test --workspace --exclude lazymkv-ocr --locked --offline --no-fail-fast`; when another
  session is also testing, add `-- --test-threads=2`. The engine's timing tests can fail on a busy Mac on `main` too;
  rerun that binary alone before calling it a failure.
- The controller makes every commit (signed, the owner's noreply identity, no agent attribution); a task's commit step
  is the controller's. Nothing is pushed and no pull request is opened without the owner's word.
- Nothing is built on, written to or changed on the mini PC. Nothing is written to the share: publishing the owner's
  maps there is not part of this plan.

## Review Focus

1. An off file saved on Windows: a byte-order mark and `\r\n` line ends. The third field must not keep the `\r`, or
   every line is stale. Test in Task 1.
2. A choice whose text holds `|`, runs of spaces or non-ASCII letters: a line that `menu off` prints, saved as the off
   file, must parse and fit its choice. Test in Task 5.
3. A label holding a double quote or a control character: the note's detail stays one line. Test in Task 3.
4. Publishing the same map twice, and over a folder that holds an older pair: both files are replaced, no temporary
   file is left, and an off file the map no longer has is removed. Test in Task 5.
5. `--to` naming a file, or a root whose folder for this disc ID is a file: refused with nothing written. Test in
   Task 5.

---

### Task 1: The off file's third field

**Files:**
- Modify: `crates/core/src/menu/role.rs` (`OffDecisions`, `parse_off`, `stream_role`, its tests module)
- Modify (call sites of the old two-field form, tests only): `crates/core/src/menu/tie.rs` (near lines 1125, 1147,
  1181), `crates/dev/src/cmd/menu.rs` (near 768 to 800), `crates/dev/src/cmd/decide.rs` (near 3922 to 3957)

**Interfaces:**
- Produces:
  - `pub fn parse_off(text: &str) -> Result<OffDecisions, Vec<String>>`: every decision line is
    `<decision><TAB><choice id><TAB><text>`.
  - `impl OffDecisions`: `pub fn fits(&self, id: &str, text: &str) -> Option<OffDecision>` (the decision on `id`, only
    when its line's text equals `text` code point for code point); `pub fn text(&self, id: &str) -> Option<&str>`
    (the line's text); `pub fn ids(&self) -> Vec<&str>` as today. `get` is removed.
  - `stream_role(choices: &[(&str, &str)], off: &OffDecisions) -> RoleReading` keeps its signature and applies a
    decision through `fits(id, text)`.

- [ ] **Step 1: Write the failing tests** in `role.rs`'s tests module (replace the tests that use two-field lines).

```rust
#[test]
fn a_decision_line_names_the_wording_it_was_made_from() {
    let off = parse_off("# note\noff\tc012\tCOMMENTARY: Off\nrole\tc013\tAUDIO: Commentary by Person A\n").unwrap();
    assert_eq!(off.ids(), ["c012", "c013"]);
    assert_eq!(off.text("c012"), Some("COMMENTARY: Off"));
    assert_eq!(off.fits("c012", "COMMENTARY: Off"), Some(OffDecision::Off));
    assert_eq!(off.fits("c012", "COMMENTARY: On"), None);
    assert_eq!(off.fits("c099", "COMMENTARY: Off"), None);
}

#[test]
fn a_windows_off_file_reads_as_a_unix_one() {
    let off = parse_off("\u{feff}off\tc012\tCOMMENTARY: Off\r\nrole\tc013\tAUDIO: Été | A\r\n").unwrap();
    assert_eq!(off.fits("c012", "COMMENTARY: Off"), Some(OffDecision::Off));
    assert_eq!(off.text("c013"), Some("AUDIO: Été | A"));
}

#[test]
fn a_line_without_its_text_and_an_id_with_two_texts_are_refused() {
    let errors = parse_off("off\tc012\noff\tc012\t\noff\tc012\tA\tB\noff\tc013\tA\noff\tc013\tB\nrole\tc013\tA\n")
        .unwrap_err();
    let expected = "expected \"off<TAB><choice id><TAB><the choice's text>\" or \
                    \"role<TAB><choice id><TAB><the choice's text>\"";
    assert_eq!(
        errors,
        [
            format!("line 1: {expected}"),
            format!("line 2: {expected}"),
            format!("line 3: {expected}"),
            "line 5: c013 has another text here than on line 4".to_string(),
            "line 6: c013 is decided role here and off on line 4".to_string(),
        ]
    );
}

#[test]
fn a_decision_that_does_not_fit_its_text_is_no_decision() {
    // MR-6 with the binding: c012's line was written for other wording, so its text is undecided again.
    let off = parse_off("off\tc012\tCOMMENTARY: Off\n").unwrap();
    let now = [("c011", "AUDIO: Commentary by Person A"), ("c012", "COMMENTARY: None")];
    assert_eq!(stream_role(&now, &off), RoleReading::Undecided(vec!["c012".to_string()]));
    let then = [("c011", "AUDIO: Commentary by Person A"), ("c012", "COMMENTARY: Off")];
    assert_eq!(stream_role(&then, &off), RoleReading::Role(AudioRole::Commentary));
}
```

- [ ] **Step 2: Run them and see them fail**

Run: `$CARGO test -p lazymkv-core --locked --offline menu::role`
Expected: does not compile (`fits`, `text` not found).

- [ ] **Step 3: Implement.** `OffDecisions` holds, by id, the decision and the line's text. `parse_off` splits a line
  on `\t` into exactly three fields, the third not empty; the unknown-decision and bad-id sentences stay as they are;
  a repeated line (same decision, id and text) is dropped; a second line for an id with another decision gives
  today's "decided ... here and ... on line" sentence, and with the same decision and another text
  `line <n>: <id> has another text here than on line <m>`. `stream_role` uses `off.fits(id, text)`.

- [ ] **Step 4: Update the two-field call sites** named under Files to the three-field form, giving each line the
  `role_text` of the choice it names in that test's map (for the shared fixture: c002 is `AUDIO: Language B 5.1`, c003
  `AUDIO: COMMENTARY 1`; a test that relabels c002 uses `AUDIO: <its label>`; an id no choice has, such as c099, takes
  any text). Their assertions on refusal sentences take the new "expected" sentence.

- [ ] **Step 5: Run the two crates' tests**

Run: `$CARGO test -p lazymkv-core -p lazymkv-dev --locked --offline`
Expected: PASS, no warning.

- [ ] **Step 6: Commit** (controller): `menu maps: an off-decision names the wording it was made from`.

---

### Task 2: Stale decisions and Play's stream in the roles block

**Files:**
- Modify: `crates/core/src/menu/tie.rs` (`RolesReport`, `RolesBlock`, `RolesReport::block`, `unused_decisions`,
  `audio_roles`, tests)

**Interfaces:**
- Consumes: `OffDecisions::text`, `OffDecisions::ids` (Task 1).
- Produces:
  - `RolesReport` and `RolesBlock` each gain, after `off_unused`: `pub off_stale: Vec<String>` and
    `pub play: Option<u32>`. On `RolesBlock` both carry `#[serde(default)]` (EP-1).
  - `off_stale`: the decided ids, ascending, for which the map has an audio choice with that id whose `role_text`
    differs from the line's text. `off_unused`: the decided ids that are not stale and name no audio choice of the
    map, or one whose text holds no role word. No id is in both.
  - `play`: the scan stream plain Play selects, the `play_stream` that `audio_roles` already computes (MR-17); `None`
    when it is not known and when the title is untied.

- [ ] **Step 1: Write the failing tests** in `tie.rs`'s tests module, beside the `off_unused` tests.

```rust
#[test]
fn a_decision_whose_text_no_longer_fits_is_stale_and_one_that_names_no_choice_is_unused() {
    // The map's c010 reads "AUDIO: Commentary Off"; the line for it was written for other wording.
    let off = parse_off("off\tc010\tAUDIO: Commentary None\noff\tc099\tAUDIO: Anything\n").unwrap();
    let report = audio_roles(&map_with_off_choice(), &off, &title(), &playlists());
    assert_eq!(report.off_stale, ["c010"]);
    assert_eq!(report.off_unused, ["c099"]);
    let row = report.audio.iter().find(|row| row.choices.contains(&"c010".to_string())).unwrap();
    assert_eq!((row.role, row.why.as_str()), (Option::None, "undecided: c010 (menu-off.tsv)"));
}

#[test]
fn the_block_says_which_stream_plain_play_selects() {
    let block = roles(&map(), &title(), &playlists()).block(&matching());
    assert_eq!(block.play, Some(1));
    let untied = roles(&map(), &title(), &BTreeMap::new()).block(&matching());
    assert_eq!(untied.play, Option::None);
    let want = r#"{"acts":false,"why":"playlist 00800 was not read","audio":[],"unplaced":[],"off_unused":[],"off_stale":[],"play":null}"#;
    assert_eq!(serde_json::to_string(&untied).unwrap(), want);
    let old: RolesBlock = serde_json::from_str(r#"{"acts":false,"why":"x","audio":[],"unplaced":[],"off_unused":[]}"#).unwrap();
    assert_eq!((old.off_stale, old.play), (Vec::new(), Option::None));
}
```

  `map_with_off_choice`, `map`, `title`, `playlists` and `matching` stand for the module's existing helpers (the
  made-up map with an off choice near line 1142, `roles` at line 706, the playlists of the `play` tests near line
  1237); use them under their real names. Replace the serialised expectation near line 1208 with `want` above.

- [ ] **Step 2: Run** `$CARGO test -p lazymkv-core --locked --offline menu::tie` and see both fail to compile.

- [ ] **Step 3: Implement** the two fields, the split of `unused_decisions` into stale and unused, and `play` from
  `play_stream`.

- [ ] **Step 4: Run** `$CARGO test -p lazymkv-core -p lazymkv-dev --locked --offline`. Expected: PASS. A `dev` test
  that compares a whole `roles` block with a JSON literal gains `"off_stale": []` and its `"play"` value.

- [ ] **Step 5: Commit** (controller): `menu maps: the roles block lists stale decisions and Play's stream`.

---

### Task 3: The rule: the notes, a title without a reference, a doubted and a disputed reference

**Files:**
- Modify: `crates/core/src/review.rs` (`Note`, `Note::ALL`, `as_str`, the `round_trips!` list)
- Modify: `crates/core/src/tracks/audio.rs` (`AudioFacts`, `MenuConfirmation`, `MenuOutcome`, `decide_audio`,
  `menu_outcomes`)
- Modify: `crates/core/src/tracks/audio/tests.rs` (the menu tests from line 1465 on)
- Modify: `crates/dev/src/cmd/decide.rs` line 954 only, so the workspace compiles: `labels: Vec::new()` until Task 4

**Interfaces:**
- Produces:
  - `Note::MenuDropped` (`menu_dropped`) and `Note::MenuReferenceConflict` (`menu_reference_conflict`), appended
    after `UnlistedLabelCut` in that order; `Note::ALL` is `[Note; 16]`.
  - `MenuConfirmation { pub role: AudioRole, pub choices: Vec<String>, pub labels: Vec<String> }`: `labels` are the
    shown labels of `choices`, in the same order.
  - `AudioFacts` gains `pub menu_play: Option<u32>`: the scan stream plain Play selects, `None` when not known.
  - `MenuOutcome` in this order: `Dropped`, `NotKept`, `Overridden`, `Reference`, `ReferenceDisputed`
    (`reference_disputed`), `NoReference` (`no_reference`), `ReferenceWouldChange`, `WouldLeaveNone`.
- The rule, after the first arrangement and the per-confirmation outcomes (`NotKept`, `Overridden`, `Reference`, else
  a candidate), with EP-8's order:
  1. any confirmation of the title has the outcome `Reference`: every candidate is `ReferenceDisputed`;
  2. the first arrangement has no reference, and `menu_play` is `None` or is not a kept stream: every candidate is
     `NoReference`;
  3. `WouldLeaveNone`, 4. `ReferenceWouldChange`, as today; 5. else the candidates are `Dropped`.
- Reviews: `AudioReferenceUnclear` is added when any outcome is `Reference` or `ReferenceWouldChange`, and, after a
  drop, when the first arrangement's reviews held it.
- Notes, appended after the arrangement's own notes, ascending stream. `<role>` is `a commentary` or `an audio
  description`; `<labels>` is each label in double quotes, joined with `, `, a label that `menu::line_text` would
  escape written as `line_text` writes it (EP-6):
  - each `Dropped` stream `n`: `MenuDropped`, `stream: Some(n)`,
    `the disc's menu calls stream <n> <role> (<labels>); dropped on the menu's word`;
  - each `Reference` stream `n`: `MenuReferenceConflict`, `stream: Some(n)`,
    `the disc's menu calls stream <n>, the programme reference, <role> (<labels>); nothing was dropped on the menu's word`;
  - `ReferenceWouldChange`: one `MenuReferenceConflict`, `stream: None`,
    `dropping the tracks the menu confirms (streams 3, 4) would change the programme reference from 1 to 5; none dropped`,
    with `(stream 3)` for one candidate and `none` for a reference that would be lost.

- [ ] **Step 1: Write the failing tests.** In `review.rs` add the two `round_trips!` lines
  (`note_menu_dropped_round_trips: Note::MenuDropped => "menu_dropped"`, and the like). In `audio/tests.rs`, give
  `confirmed` a third argument and add:

```rust
fn confirmed(role: AudioRole, choices: &[&str], labels: &[&str]) -> MenuConfirmation { /* three Vecs */ }
// menu_facts(): c010 labelled "AUDIO: Commentary by Person A", c011 "AUDIO: Descriptive Audio".

#[test]
fn a_menu_drop_raises_a_note_with_the_menus_wording() {
    let decision = decide_with(&menu_title(), &menu_facts());
    let notes: Vec<&Noted> = decision.notes.iter().filter(|noted| noted.note == Note::MenuDropped).collect();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].stream, Some(2));
    assert_eq!(
        notes[0].detail,
        "the disc's menu calls stream 2 a commentary (\"AUDIO: Commentary by Person A\"); dropped on the menu's word"
    );
    assert_eq!(
        notes[1].detail,
        "the disc's menu calls stream 3 an audio description (\"AUDIO: Descriptive Audio\"); dropped on the menu's word"
    );
    assert_eq!(decision.reviews, Vec::new());
}

#[test]
fn a_label_that_would_break_a_line_is_escaped_in_the_note() {
    let mut facts = menu_facts();
    facts.menu.insert(2, confirmed(AudioRole::Commentary, &["c010", "c012"], &["A \"quoted\" one", "two\nlines"]));
    let decision = decide_with(&menu_title(), &facts);
    let detail = &decision.notes.iter().find(|noted| noted.stream == Some(2) && noted.note == Note::MenuDropped).unwrap().detail;
    assert!(!detail.contains('\n'), "{detail}");
    assert!(detail.contains("\"A \"quoted\" one\", ") && detail.contains("two\\nlines"), "{detail}");
}

#[test]
fn a_title_without_a_reference_drops_only_when_plays_stream_is_known_and_kept() {
    // Two kept streams with no language code: no reference (spec 8.6 "Content"). The menu confirms stream 2.
    let title = no_language_title(); // streams 1 and 2, both without a language code
    let menu = BTreeMap::from([(2, confirmed(AudioRole::Commentary, &["c010"], &["AUDIO: Commentary"]))]);
    for (play, outcome, kept) in [
        (Option::None, MenuOutcome::NoReference, vec![1, 2]),
        (Some(9), MenuOutcome::NoReference, vec![1, 2]),
        (Some(1), MenuOutcome::Dropped, vec![1]),
    ] {
        let decision = decide_with(&title, &AudioFacts { menu: menu.clone(), menu_play: play, ..AudioFacts::default() });
        assert_eq!(decision.reference, Option::None);
        assert_eq!(outcomes(&decision), vec![(2, outcome)], "{play:?}");
        assert_eq!(order_of(&decision), kept, "{play:?}");
    }
}

#[test]
fn a_doubted_reference_survives_the_menu_drop() {
    let facts = AudioFacts { verdicts: BTreeMap::from([(2, Verdict::ReferenceUnclear), (3, Verdict::Keep)]), ..menu_facts() };
    let decision = decide_with(&menu_title(), &facts);
    assert_eq!(outcomes(&decision), vec![(2, MenuOutcome::Dropped), (3, MenuOutcome::Dropped)]);
    assert_eq!(decision.reviews, vec![ReviewReason::AudioReferenceUnclear]);
    // A commentary verdict on a dropped stream still goes with it (the owner's "Yes, drop it").
    let facts = AudioFacts { verdicts: BTreeMap::from([(2, Verdict::Commentary), (3, Verdict::Keep)]), ..menu_facts() };
    assert_eq!(decide_with(&menu_title(), &facts).reviews, Vec::new());
}

#[test]
fn a_confirmation_on_the_reference_holds_the_job_and_drops_nothing_on_the_title() {
    let mut facts = menu_facts();
    facts.menu.insert(1, confirmed(AudioRole::Commentary, &["c001"], &["AUDIO: Commentary by Person B"]));
    let decision = decide_with(&menu_title(), &facts);
    assert_eq!(
        outcomes(&decision),
        vec![(1, MenuOutcome::Reference), (2, MenuOutcome::ReferenceDisputed), (3, MenuOutcome::ReferenceDisputed)]
    );
    assert_eq!(order_of(&decision), vec![1, 2, 3]);
    assert!(decision.dropped.iter().all(|(_, why)| !matches!(why, AudioDrop::Menu { .. })));
    assert!(decision.reviews.contains(&ReviewReason::AudioReferenceUnclear));
    let note = decision.notes.iter().find(|noted| noted.note == Note::MenuReferenceConflict).unwrap();
    assert_eq!(note.stream, Some(1));
    assert_eq!(
        note.detail,
        "the disc's menu calls stream 1, the programme reference, a commentary (\"AUDIO: Commentary by Person B\"); \
         nothing was dropped on the menu's word"
    );
    assert!(decision.notes.iter().all(|noted| noted.note != Note::MenuDropped));
}

#[test]
fn a_keep_override_on_the_reference_disputes_nothing() {
    // Stream 1 is the reference, the menu confirms it, and a remembered Keep names it: `overridden`, so the title's
    // other candidates are dropped and nothing is held.
    // Build the Keep as the existing test a_keep_override_wins_over_the_menu does.
    // outcomes == [(1, Overridden), (2, Dropped), (3, Dropped)]; reviews hold no AudioReferenceUnclear.
}
```

  `no_language_title` is a new helper beside `menu_title`: a video stream and two Dolby Digital audio streams with no
  language code.

  Also extend these existing tests rather than adding twins:
  - `a_menu_commentary_and_a_menu_audio_description_are_dropped_where_content_kept_them`: its "no note names stream 2
    or 3" assertion becomes "the only notes on streams 2 and 3 are `MenuDropped`".
  - `a_menu_drop_is_written_with_its_why_and_role`: the outcome list gains `(ReferenceDisputed, "reference_disputed")`
    and `(NoReference, "no_reference")`.
  - the `ReferenceWouldChange` test near line 1799: also `reviews` holds `AudioReferenceUnclear`, and one
    `MenuReferenceConflict` note with `stream: None` and the sentence above with that test's streams.
  - the two `WouldLeaveNone` tests near lines 1847 and 1857: set `menu_play` to a kept stream and confirm that stream
    too, so the guard is reached (EP-7); a comment says the wiring never gives such a confirmation.
  - `without_confirmations_the_decision_is_todays` must pass unchanged.

- [ ] **Step 2: Run** `$CARGO test -p lazymkv-core --locked --offline tracks::audio review` and see the new ones fail.

- [ ] **Step 3: Implement** in `review.rs` and `audio.rs` as the Interfaces block says. Keep `arrange` as it is;
  the second arrangement is made only when rules 1 and 2 did not refuse. The doc comments of `decide_audio`,
  `MenuOutcome` and `MenuConfirmation` say the rule as it now is and name EO-3, EO-4, EO-5 and EO-11.

- [ ] **Step 4: Run** `$CARGO test -p lazymkv-core -p lazymkv-dev --locked --offline`. Expected: PASS. An exhaustive
  `match` on `Note` or `MenuOutcome` that the compiler names gets its new arms.

- [ ] **Step 5: Commit** (controller): `menu maps: the menu's drop raises a note, and three reference rules`.

---

### Task 4: decide's wiring: labels, Play's stream, the shared reader

**Files:**
- Modify: `crates/dev/src/cmd/decide.rs` (`decision_section` near line 947, `run` near line 831, `menu_input` at
  line 621, the `--menu` tests from line 3668 on)
- Modify: `docs/phase1/decision-core.md` (the decide section's `--menu` paragraph, near line 181)

**Interfaces:**
- Consumes: `RolesBlock.play`, `off_stale` (Task 2); `MenuConfirmation.labels`, `AudioFacts.menu_play` (Task 3).
- Produces:
  - `pub fn read_menu_pair(ctx: &Ctx, given: &str) -> Result<(MenuMap, OffDecisions), Refused>`: `read_map` then
    `read_off` of the same `given`, the refusal as today (code 2, one line per problem).
  - `pub fn menu_input(rec: &Recording, map: MenuMap, off: OffDecisions, inputs: &DecideInputs) -> MenuInput`: today's
    private function, made public. `run` calls the two in today's order (EP-3).
  - `decision_section` gives each `MenuConfirmation` its `labels` (`Choice::shown_label` of each id in the row's
    `choices`, in that order) and sets `AudioFacts.menu_play` to the block's `play` when the block acts, else `None`.

- [ ] **Step 1: Write the failing tests** in the `--menu` test block.

```rust
#[test]
fn a_menu_drop_prints_its_note_and_plays_stream() {
    let tmp = Scratch::new("menu-note");
    let root = tmp.0.clone();
    role_world(&root, Some(MAP_ID), true, "Commentary");
    let (plain, mapped) = keep_inputs(&root);
    let (without, with) = (printed_value(&plain), printed_value(&mapped));
    let notes = with["decision"]["decision"]["notes"].as_array().unwrap();
    let dropped: Vec<&Value> = notes.iter().filter(|note| note["note"] == json!("menu_dropped")).collect();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0]["stream"], json!(3));
    assert_eq!(
        dropped[0]["detail"],
        json!("the disc's menu calls stream 3 a commentary (\"SETUP > AUDIO: Commentary\"); dropped on the menu's word")
    );
    assert_eq!(with["menu"]["roles"]["play"], json!(1));
    assert_eq!(with["menu"]["roles"]["off_stale"], json!([]));
    assert!(without["decision"]["decision"]["notes"].as_array().unwrap().iter().all(|n| n["note"] != json!("menu_dropped")));
}

#[test]
fn a_stale_off_line_is_listed_and_decides_nothing() {
    // c002 reads "AUDIO: Commentary Off": the net catches it. A line written for other wording does not apply.
    let tmp = Scratch::new("menu-off-stale");
    let root = tmp.0.clone();
    role_world(&root, Some(MAP_ID), true, "Commentary Off");
    write(&root.join(MAP_DIR).join(menu::OFF_FILE_NAME), "role\tc002\tAUDIO: Commentary None\n");
    let (code, out, err) = run_in(&root, &["recs/disc-b", "all-tracks", "--menu", MAP_DIR]);
    assert_eq!((code, err.as_str()), (0, ""));
    let ran: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(ran["menu"]["roles"]["off_stale"], json!(["c002"]));
    assert!(ran["decision"]["decision"]["audio"].get("menu").is_none());
    // The line as it should be: the choice is read as a role, and the stream is dropped.
    write(&root.join(MAP_DIR).join(menu::OFF_FILE_NAME), "role\tc002\tAUDIO: Commentary Off\n");
    let (_, out, _) = run_in(&root, &["recs/disc-b", "all-tracks", "--menu", MAP_DIR]);
    let ran: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(ran["decision"]["decision"]["audio"]["menu"][0]["outcome"], json!("dropped"));
}

#[test]
fn a_two_field_off_file_is_refused_with_exit_2() {
    let tmp = Scratch::new("menu-off-two-fields");
    let root = tmp.0.clone();
    role_world(&root, Some(MAP_ID), true, "Commentary Off");
    write(&root.join(MAP_DIR).join(menu::OFF_FILE_NAME), "off\tc002\n");
    let (code, out, err) = run_in(&root, &["recs/disc-b", "all-tracks", "--menu", MAP_DIR]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.contains("menu-off.tsv: line 1: expected \"off<TAB><choice id><TAB><the choice's text>\""), "{err}");
}
```

- [ ] **Step 2: Run** `$CARGO test -p lazymkv-dev --locked --offline cmd::decide::tests` and see the first two fail.

- [ ] **Step 3: Implement** the Interfaces block. In `decision_section` the labels come from the map of
  `inputs.menu` (`given.map.choices`), looked up by id.

- [ ] **Step 4: Run** `$CARGO test -p lazymkv-dev --locked --offline`. Expected: PASS; the characterisation goldens
  (which use no map) are untouched: `git status` shows no change under `crates/dev/src/cmd/decide/characterisation/`.

- [ ] **Step 5: Reword** `docs/phase1/decision-core.md`'s `--menu` paragraph for the note, the two new outcomes and
  the three-field off file.

- [ ] **Step 6: Commit** (controller): `decide --menu: the note's wording, Play's stream, one shared reader`.

---

### Task 5: `menu off` and `menu publish`

**Files:**
- Modify: `crates/core/src/menu/mod.rs` (two constants), `crates/core/src/menu/scan.rs` (`disc_folder`, with a test in
  `crates/core/src/menu/tests.rs`)
- Modify: `crates/dev/src/cmd/menu.rs` (usage and help texts, `run`, two new verbs, tests)

**Interfaces:**
- Consumes: `OffDecisions::fits`, `text`, `ids` (Task 1); `read_map`, `read_off` (this file).
- Produces, in `core::menu`:
  - `pub const MAP_MAX_BYTES: u64 = 4 * 1024 * 1024;` and `pub const OFF_MAX_BYTES: u64 = 1024 * 1024;`
  - `pub fn disc_folder(id: &str) -> Option<String>`: `id` trimmed; `Some` of its upper-case form when it is exactly
    40 ASCII hex digits, else `None`.
- Produces, in `lazymkv-dev menu`:
  - `USAGE` is `menu [-h] {show,check,off,publish} ...`; `OFF_USAGE` is `menu off [-h] map`; `PUBLISH_USAGE` is
    `menu publish [-h] map --to ROOT`. The invalid-choice line names the four verbs.
  - `menu off <map>`: for each audio choice of the map, in map order, whose `role_text` holds a role word and that the
    net catches (`needs_decision`) or a decision names: one line `<state><TAB><id><TAB><text>`, `<text>` the choice's
    `role_text` and `<state>` one of `off`, `role` (a decision that fits), `stale` (a decision for that id with
    another text), `undecided`. Exit 0; 2 for a map or off file it refuses.
  - `menu publish <map> --to ROOT`: spec part 8. `ROOT` is taken against the context's folder when relative. Refusals,
    each one `lazymkv-dev menu: error: <line>` and exit 2, with nothing written:
    - the map's and the off file's problems, as `show` and `decide --menu` print them;
    - `<map shown>: the map has no disc ID, so no job could find it` (`disc_folder` gives `None`);
    - `<off shown>: <id> is stale: the map's text is <repr>, the line's is <repr>` for each stale line, the two texts
      as Python's `repr` writes them (EO-12);
    - `<shown>: <n> bytes is over the limit of <limit>` for either file;
    - `<ROOT>: not a folder`; `<ROOT>/<ID>: not a folder` when that name is a file.
  - On success it makes `<ROOT>/<ID>/` when missing, writes the source files' bytes unchanged to
    `<ROOT>/<ID>/menu-map.json` and, when the map has an off file, `<ROOT>/<ID>/menu-off.tsv`, each with
    `lazymkv_platform::write_atomic`; removes a `menu-off.tsv` there when the map has none; and prints one line,
    `PUBLISHED <ID> choices=<n> decisions=<m> undecided=<k>` (`n` the map's choices, `m` the decisions that fit, `k`
    the `undecided` lines `menu off` would print). An unused line is no refusal (EO-13).

- [ ] **Step 1: Write the failing tests** in `menu.rs`'s tests module (helpers `world`, `menu_run`, `MAP_DIR`,
  `FIXTURE_ID` exist) and `disc_folder`'s in `core`.

```rust
// crates/core/src/menu/tests.rs
#[test]
fn a_disc_folder_is_forty_hex_digits_upper_cased() {
    let id = "0123456789abcdef0123456789ABCDEF01234567";
    assert_eq!(disc_folder(&format!(" {id}\n")), Some(id.to_uppercase()));
    for bad in ["", "0123", &id[1..], &format!("{id}0"), &id.replace('0', "g"), &id.replace('1', "/")] {
        assert_eq!(disc_folder(bad), None, "{bad:?}");
    }
}

// crates/dev/src/cmd/menu.rs: `tricky_world(tag)` writes MAP_DIR's map as the fixture with c002 labelled
// "Commentary  |  Été   Off" and c003 labelled "Commentary: None", both at level replayed.
#[test]
fn menu_off_lists_every_choice_the_net_catches_with_its_state() {
    let tmp = tricky_world("menu-off-list");
    let (code, out, err) = menu_run(&tmp, &["off", MAP_DIR]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, "undecided\tc002\tAUDIO: Commentary / Été Off\nundecided\tc003\tAUDIO: Commentary: None\n");
    tmp.write(&format!("{MAP_DIR}/{OFF_FILE_NAME}"), "off\tc002\tAUDIO: Commentary / Été Off\nrole\tc003\tAUDIO: Other\n");
    let (_, out, _) = menu_run(&tmp, &["off", MAP_DIR]);
    assert_eq!(out, "off\tc002\tAUDIO: Commentary / Été Off\nstale\tc003\tAUDIO: Commentary: None\n");
}

#[test]
fn a_line_menu_off_prints_fits_its_choice_when_saved_as_the_off_file() {
    let tmp = tricky_world("menu-off-round-trip");
    let (_, listed, _) = menu_run(&tmp, &["off", MAP_DIR]);
    tmp.write(&format!("{MAP_DIR}/{OFF_FILE_NAME}"), listed.replace("undecided\t", "off\t"));
    let (code, out, err) = menu_run(&tmp, &["off", MAP_DIR]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.lines().all(|line| line.starts_with("off\t")), "{out}");
}

#[test]
fn menu_publish_copies_the_pair_under_the_disc_id() {
    let tmp = tricky_world("menu-publish");
    tmp.write(&format!("{MAP_DIR}/{OFF_FILE_NAME}"), "off\tc002\tAUDIO: Commentary / Été Off\noff\tc099\tAUDIO: Gone\n");
    fs::create_dir_all(tmp.0.join("root/OTHER")).unwrap();
    tmp.write("root/OTHER/menu-map.json", "untouched");
    let (code, out, err) = menu_run(&tmp, &["publish", MAP_DIR, "--to", "root"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(out, format!("PUBLISHED {FIXTURE_ID} choices=9 decisions=1 undecided=1\n"));
    let folder = tmp.0.join("root").join(FIXTURE_ID);
    assert_eq!(fs::read(folder.join(FILE_NAME)).unwrap(), fs::read(tmp.0.join(MAP_DIR).join(FILE_NAME)).unwrap());
    assert_eq!(fs::read(folder.join(OFF_FILE_NAME)).unwrap(), fs::read(tmp.0.join(MAP_DIR).join(OFF_FILE_NAME)).unwrap());
    assert_eq!(fs::read_to_string(tmp.0.join("root/OTHER/menu-map.json")).unwrap(), "untouched");
    // Again, now without an off file: the map is replaced, the old off file goes, and nothing else is left behind.
    fs::remove_file(tmp.0.join(MAP_DIR).join(OFF_FILE_NAME)).unwrap();
    let (code, out, _) = menu_run(&tmp, &["publish", MAP_DIR, "--to", "root"]);
    assert_eq!((code, out), (0, format!("PUBLISHED {FIXTURE_ID} choices=9 decisions=0 undecided=2\n")));
    let names: Vec<String> = fs::read_dir(&folder).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    assert_eq!(names, [FILE_NAME]);
}

#[test]
fn menu_publish_refuses_and_writes_nothing() {
    // One world per row; after each run `root` holds no folder for FIXTURE_ID.
    // (what is wrong, the line stderr must hold)
    // - the map's disc_id_sha1 is null            -> "the map has no disc ID, so no job could find it"
    // - a stale off line for c002                 -> "c002 is stale: the map's text is 'AUDIO: Commentary / Été Off', the line's is 'AUDIO: Other'"
    // - a two-field off line                      -> "line 1: expected \"off<TAB><choice id><TAB><the choice's text>\""
    // - the map followed by MAP_MAX_BYTES spaces  -> "bytes is over the limit of 4194304"
    // - an invalid map (as menu_show_lists_every_problem_of_an_invalid_map_and_exits_2 builds it)
    // - `root` is missing, and `root` is a file   -> "root: not a folder"
    // - `root/<FIXTURE_ID>` is a file             -> "not a folder"
    // each: exit 2, stdout empty.
}

#[test]
fn menu_publish_names_the_folder_in_upper_case_for_a_lower_case_id() {
    // The map's disc_id_sha1 in lower case: the folder is FIXTURE_ID, upper case.
}
```

  Extend `an_unknown_verb_is_a_usage_error_with_exit_2`, `missing_and_extra_arguments_are_usage_errors_with_exit_2`
  and `help_before_or_after_a_verb_prints_that_usage_and_exits_0` for the two verbs (`publish` without `--to` is
  `the following arguments are required: --to`).

- [ ] **Step 2: Run** `$CARGO test -p lazymkv-core -p lazymkv-dev --locked --offline menu` and see them fail.

- [ ] **Step 3: Implement** `disc_folder` and the constants, then `run_off` and `run_publish` beside `run_show`, and
  their help texts in the style of `SHOW_HELP`. The sizes are read from the files' metadata before the files are read.

- [ ] **Step 4: Run** `$CARGO test -p lazymkv-core -p lazymkv-dev --locked --offline`. Expected: PASS, no warning.

- [ ] **Step 5: Commit** (controller): `lazymkv-dev menu: list a map's off-decisions, and publish a map`.

---

### Task 6: The documents

**Files:**
- Modify: `docs/menu-maps/format.md`
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (8.6 step 1's menu bullet, step 6, "Content"; 8.8's
  table row for `audio_reference_unclear`)
- Modify: `docs/superpowers/specs/2026-11-17-menu-maps-design.md` (its status line: a pointer to the engine menu maps
  design and this pull request)

- [ ] **Step 1: `format.md`.** In place, dated 2026-11-24, with nothing about the engine changed (the engine still
  gives no map):
  - "The off-decisions file": the three-field line, when a decision applies, a stale line, the new refusal sentences,
    and the made-up example in the new form; the paragraph saying the file "is not bound to the map" is replaced.
  - "How decide shows a map": the `roles` keys `off_stale` and `play` (EP-1) and the made-up `roles` example with
    them.
  - "Roles": the `why` form `undecided: <ids> (menu-off.tsv)` also covers a stale line.
  - "The rule": the eight outcomes in EP-8's order with what each means; the title without a reference; the review
    reason kept after a drop; the reason and the note for `reference` and `reference_would_change`; the note
    `menu_dropped`; MR-11's "no note" replaced, its "no review" kept; the made-up example of a title with no language
    codes now ends in `no_reference`.
  - "The folder and the file" and "Exit codes": `menu off` and `menu publish` with their lines and exit codes, the
    size limits and `disc_folder`.
  - "Rulings": the owner's answers that this pull request builds (EO-3, EO-4, EO-5, EO-11, EO-12, EO-13, quoted as
    the design quotes them), the designer's calls it builds (EM-3, EM-9, EM-10, EM-19, EM-22) and EP-1 to EP-9.
  - "Tests and acceptance": a new record, filled from Task 7.

- [ ] **Step 2: The spec.** 8.6 step 1's menu bullet names the two new conditions (no drop on a title whose reference
  the menu confirms; on a title without a reference, only when Play's stream is known and kept) and keeps "The engine
  gives no map yet". Step 6 gains the two notes. "Content" and 8.8's row say `audio_reference_unclear` is also raised
  by the menu against the reference and kept through a menu drop. Each addition is dated 2026-11-24.

- [ ] **Step 3: Check** that no document still says the old thing:

Run: `grep -rn "not bound to the map\|raises no note\|six outcomes\|off<TAB><choice id>\"" docs/menu-maps docs/phase1 docs/superpowers/specs/2026-10-04-lazymkv-design.md`
Expected: no line that states today's behaviour as the rule; a hit inside a dated ruling quoted verbatim (MR-11, MA-3)
is left as it is, followed by the sentence that replaces it.

- [ ] **Step 4: Commit** (controller): `docs: the menu rule and the off-decisions file as pull request 1 builds them`.

---

### Task 7: Acceptance stage A, on the Mac (the controller's own task)

Private data: read `~/lazymkv-menu-spike/`, write only under `~/lazymkv-menu-spike/out/engine-menu-maps/`. Nothing of
it is committed (EP-9). No sub-agent reads the private data; a checker gets counts-only copies.

- [ ] **Step 1: Build** `$CARGO build --release --locked --offline -p lazymkv-dev` and run the full test command of
  Global Constraints. Record the counts.
- [ ] **Step 2: Rebind the 13 off files.** For each map copy under `out/menu-role/maps/<slug>/`, copy the folder to
  `out/engine-menu-maps/maps/<slug>/`, run `lazymkv-dev menu off` on it, and write each existing decision in the
  three-field form with the text the listing prints for its id. Expected: 14 decisions carried over (8 `off`, 6
  `role`), no `stale` line, and the one text the last record left undecided still `undecided`.
- [ ] **Step 3: Publish locally.** `lazymkv-dev menu publish <map> --to out/engine-menu-maps/root` for each of the
  13. Expected: 13 `PUBLISHED` lines, 13 folders named by 40 upper-case hex digits, each pair byte-equal to its source.
- [ ] **Step 4: Decide.** `lazymkv-dev decide` without and with `--menu` on the 13 recording copies of the last
  record (`out/menu-role/rec/<slug>`), the map taken from its published folder. Expected, from the format's "Menu
  alone" record: exit codes equal on 13 of 13; every key but `decision` and `menu` byte-equal on 13 of 13; 30
  confirmations, 30 `dropped` on 11 discs; the programme reference the same before and after on each; 30
  `menu_dropped` notes; against the run without a map `commentary_detected` goes from 4 discs and no other review
  reason changes; no `reference_disputed`, `no_reference` or `menu_reference_conflict`.
- [ ] **Step 5: Report.** Whatever the counts are, they go into `format.md`'s new record (counts and slugs only) and
  to the owner. A count that differs from Step 4's expectation is reported with its reason, and no rule is changed to
  make it fit.
- [ ] **Step 6: Commit** (controller) the record: `docs: acceptance of the menu rule on the mapped discs`.

Publishing the 13 maps to the share is not a step of this plan. It follows at the owner's word, with the share mounted
on the Mac.
