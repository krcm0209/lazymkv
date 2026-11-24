# Menu alone: the widened menu-role rule, implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The menu's word alone drops an audio track it confirms as a commentary or an audio description: the content
rule's verdict no longer gates the drop.

**Architecture:** One change in `tracks::audio::decide_audio`: the outcome of a confirmation no longer reads the
content verdicts. The ties, the roles and `decide --menu`'s wiring are untouched. The documents are reworded to match.

**Tech Stack:** Rust 1.100.0, the workspace's crates only. No new crate.

**Spec:** `docs/superpowers/specs/2026-11-17-menu-maps-design.md` section 5.4a, with the owner's rulings of 2026-11-24
made after pull request #35 merged (below). The rule as built so far: `docs/menu-maps/format.md`, "The rule".

## The owner's rulings (2026-11-24, after the merge of #35)

- Asked "The menu says a track is a commentary (strong tie) and lazymkv's content check ALSO says commentary. Today
  that track is kept: 6 send the job to review, 6 only get a note. Should the menu's word drop it?", the owner chose
  "Yes, drop it".
- Asked "The menu says a track is a commentary or audio description (strong tie) and lazymkv's content check has NO
  verdict on it (not measured, or the measurement could not be used). Should the menu's word alone drop it?", the
  owner chose "Yes, menu alone".
- Asked whether to keep the six guards (the map's disc ID must match; the title's own playlist must be read and agree
  with the scan; an "off" wording with no recorded decision gives no role; never the stream plain Play selects; never
  the programme reference, and nothing is dropped if a drop would move it; a Keep override wins), with a track that
  has no language code treated like any other, the owner chose "Keep all six".

## Rulings of this plan (the owner can correct them)

- MA-1: the two answers together mean the content rule's verdict does not gate the menu's drop in any state: keep,
  commentary, cross-language commentary, a noted commentary or audio description, the audio-description pattern,
  not judged, reference unclear, D21's veto holding, another reference than the measured one, or no verdict at all.
  This replaces MR-2's verdict conditions and MR-12.
- MA-2: a drop that would leave the title with no kept audio track is not made: none of the title's confirmed streams
  is dropped (outcome `would_leave_none`). Now that a track with no language code can be dropped, a title without a
  programme reference could otherwise lose its only audio.
- MA-3: the decision's list of confirmations no longer carries a content verdict; the outcomes are `dropped`,
  `not_kept`, `overridden`, `reference`, `reference_would_change`, `would_leave_none`.
- MA-4: the six sentences that pull request #35 added to the main design are replaced by ones that say the rule as it
  now is; they stay dated 2026-11-24.

## Global Constraints

- Audio only. Without a map every document of every command is byte-identical to `main`. The engine gives no map.
- The guards stay exactly as built: MR-3 (a Keep override wins), MR-4 (nothing is dropped when the drop would move
  the reference), MR-5, MR-6, MR-8, MR-9, MR-13, MR-17 and "never the programme reference".
- Tests use made-up data only. No disc title, person's name, host, share or user name in any committed file.
- Builds and tests run on the Mac with the house rules' command; `Cargo.lock` does not change; no warning.
- The controller makes every commit (signed, no agent attribution). Nothing is pushed without the owner's word.

## Review Focus

1. A title whose only kept audio track the menu confirms: nothing is dropped (MA-2). Test in Task 1.
2. A confirmed stream with a Keep override, or that is the reference: still never dropped. Tests in Task 1.
3. A drop while D21's veto holds or the verdicts answer another reference: the stream is dropped and the remaining
   tracks still raise `audio_reference_unclear` as today. Test in Task 1.
4. Without confirmations: the decision is today's, byte for byte. Test in Task 1 (kept from #35).
5. A document that still says the drop needs a content verdict. Task 2's greps.

---

### Task 1: the rule without the content gate

**Files:**
- Modify: `crates/core/src/tracks/audio.rs`, `crates/core/src/tracks/audio/tests.rs`
- Modify: `crates/dev/src/cmd/decide.rs` (the `--menu` help sentence, comments, tests), `crates/core/src/menu/tie.rs`
  and `crates/core/src/menu/mod.rs` (doc comments only, where they say the drop needs the content rule's keep)

**Interfaces:**
- Produces:

```rust
/// What became of a confirmation; serde snake_case.
pub enum MenuOutcome { Dropped, NotKept, Overridden, Reference, ReferenceWouldChange, WouldLeaveNone }
/// `verdict` is removed.
pub struct MenuApplied { pub stream: u32, pub role: AudioRole, pub choices: Vec<String>, pub outcome: MenuOutcome }
```

The rule, after the first arrangement. A `Programme` confirmation is ignored and not listed. For each other
confirmation, ascending stream, the first that applies:

1. `NotKept`: the stream is not among the kept tracks.
2. `Overridden`: it has a remembered or owner's Keep override (MR-3).
3. `Reference`: it is the decision's reference.
4. Otherwise it is a candidate, whatever its language group and whatever `facts.verdicts`, `facts.veto_holds` and
   `facts.verdict_reference` hold (MA-1).

With at least one candidate the kept streams without the candidates are arranged again by the same function. Then, in
this order: when the second arrangement keeps no track, the first stands and every candidate is `WouldLeaveNone`
(MA-2); when its reference differs from the first's, the first stands and every candidate is `ReferenceWouldChange`
(MR-4); otherwise the second is the decision and every candidate is `Dropped`, listed in `dropped` as
`AudioDrop::Menu { role }` in scan order. Nothing else in `decide_audio` changes: step 1, D13, the confidence, and
what `apply_verdicts` raises for the tracks that remain.

- [ ] **Step 1: Change the tests first** in `tracks/audio/tests.rs` (the base title and helpers of the #35 tests):
  - Keep unchanged: `a_menu_commentary_and_a_menu_audio_description_are_dropped_where_content_kept_them`,
    `without_confirmations_the_decision_is_todays`, `the_reference_is_never_dropped`,
    `a_keep_override_wins_over_the_menu`, `a_stream_step_1_dropped_is_not_kept`,
    `a_programme_confirmation_is_ignored`, `a_drop_that_would_move_the_reference_is_not_made`,
    `dropping_a_stream_recomputes_first_of_language_and_the_notes`,
    `a_duplicates_named_copy_may_be_dropped_by_the_menu`.
  - `a_menu_drop_is_written_with_its_why_and_role`: the `MenuApplied` JSON has no `verdict` key in any case, and the
    six outcome spellings are `dropped`, `not_kept`, `overridden`, `reference`, `reference_would_change`,
    `would_leave_none`.
  - Replace `a_content_commentary_verdict_stays_a_review_and_is_not_dropped` by
    `a_track_content_also_calls_a_commentary_is_dropped`: verdict `Commentary` on stream 2 with the menu's commentary:
    stream 2 is dropped as `Menu`, `reviews` holds no `commentary_detected`, outcome `Dropped`.
  - Replace `the_other_verdicts_drop_nothing` by `every_content_verdict_lets_the_menu_drop`: for each of `Keep`,
    `Commentary`, `CommentaryCrossLanguage`, `NoteCommentary`, `AdPattern`, `NoteAd`, `NotJudged`, `ReferenceUnclear`,
    `Duplicate`: outcome `Dropped`, and no note or review names the dropped stream.
  - Replace `no_measured_reference_or_no_verdict_drops_nothing` by `without_a_content_verdict_the_menu_alone_drops`
    (`verdict_reference` None; and a measured title with no entry for the stream).
  - Replace `a_veto_or_another_reference_drops_nothing` by `a_veto_or_another_reference_does_not_stop_the_drop`: the
    stream is dropped, and `reviews` still holds `audio_reference_unclear` exactly as the same title gives it today
    without the menu.
  - Replace `a_no_language_stream_is_not_dropped` by `a_stream_without_a_language_code_is_dropped_like_any_other`.
  - New `a_drop_that_would_leave_no_audio_is_not_made`: a title whose only audio stream has no language code (so the
    decision has no reference) and is confirmed: kept as it was, `dropped` has no `Menu`, outcome `WouldLeaveNone`,
    and the decision equals the one without the menu apart from `menu`. A second case: two no-language streams, both
    confirmed: both `WouldLeaveNone`.
- [ ] **Step 2: Run** `CARGO_TEST -p lazymkv-core tracks::audio` — the changed tests fail (compile errors for the
  removed variants and field, then failures).
- [ ] **Step 3: Implement** in `audio.rs`; update the doc comments of `decide_audio`, `MenuOutcome`, `MenuApplied`,
  `MenuConfirmation`, `AudioFacts.menu`, `AudioDrop::Menu` and the module doc to say the rule as it now is, naming
  the owner's rulings of 2026-11-24 and MA-1 to MA-3.
- [ ] **Step 4: `crates/dev`.** The help line reads `a menu map, its folder or its menu-map.json: adds the menu key,
  and a commentary or audio description it confirms is dropped (docs/menu-maps/format.md)`. In the decide tests,
  `a_confirmed_role_drops_the_stream_and_changes_only_the_decision_and_menu_keys` no longer needs a keep verdict set
  in memory: through `run`, on the recording as read, the outcome is `dropped` and the stream is in `dropped`; keep
  the `decide_doc` half too. Every other "confirms nothing" test stays and still passes (identity not checked,
  another disc, no playlist, an undecided off text, a role on Play's own stream). Correct every comment in
  `decide.rs`, `tie.rs` and `menu/mod.rs` that says the drop needs the content rule's keep.
- [ ] **Step 5: Run** the focused tests (`-p lazymkv-core tracks::audio`, `-p lazymkv-core menu::`,
  `-p lazymkv-dev decide`, `-p lazymkv-dev menu`), then the whole workspace: no failure outside the timing-sensitive
  engine binaries, no warning, no characterisation diff.
- [ ] **Step 6: Commit** `audio: the menu's word alone drops a track it confirms`.

### Task 2: the documents

**Files:**
- Modify: `docs/menu-maps/format.md`, `docs/superpowers/specs/2026-11-17-menu-maps-design.md`,
  `docs/superpowers/specs/2026-10-04-lazymkv-design.md`, `docs/phase1/decision-core.md`

- [ ] **Step 1: `format.md`.**
  - "What a map is and is not", "How decide shows a map", "The rule": the rule as Task 1 built it (read the code): the
    conditions, the six outcomes in the order the code tests them, the JSON of a `MenuApplied` without `verdict`, the
    made-up examples corrected. Say plainly that the content rule's verdict is not asked.
  - "Rulings": keep MR-1 to MR-17 as written and add after them, dated 2026-11-24, "The owner's rulings after pull
    request #35" (the three questions and answers, quoted exactly from this plan) and MA-1 to MA-4 verbatim from this
    plan, with one sentence saying that MA-1 replaces MR-2's verdict conditions and MR-12, and that MR-11's list of
    outcomes is now MA-3's.
  - "Tests and acceptance": keep both dated records and add a subsection `### Menu alone (2026-11-24)` holding
    exactly the line `<!-- controller: test counts and acceptance on the 13 real maps go here -->`.
- [ ] **Step 2: the menu-maps design.** The status line says the rule is in `main` and names this widening. 5.4a gains
  a last paragraph, **The owner's rulings after the merge (2026-11-24)**: the three questions and answers and MA-1 to
  MA-4 as proposals. "Still unruled" loses what is now ruled (a menu role against `not-judged`, `reference-unclear`
  or a vetoed title; content's commentary or audio-description verdict against a menu role of the same or the other
  unwanted kind; MR-12's line) and keeps the rest (a menu programme role; weak ties and tie conflicts; naming or
  flagging; preferences; subtitles). In "Menu role", correct the programme words: the pattern reads `main`,
  `programm` and `programme` between word edges, not `program` (a dated correction in brackets).
- [ ] **Step 3: the main design.** Replace each of these sentences of #35 with the one given (find each by its
  opening words; change nothing else in the file):
  - R6: `Since 2026-11-24 the disc's own menu confirms it too, where a menu map is given: a commentary or
    audio-description role with a strong tie, whatever the content rule says of the track (menu maps design 5.4a).`
  - D12: `(Owner, 2026-11-21 and 2026-11-24.) A menu map's commentary or audio-description role with a strong tie
    counts as confirmed, whatever the content rule's verdict on the track and also without one: the track is dropped
    (menu maps design 5.4a; docs/menu-maps/format.md).`
  - D15: `Since 2026-11-24 a menu map's role with a strong tie also judges it, on its own word (D12).`
  - D23: `Since 2026-11-24 one source besides content is ruled: a menu map's role with a strong tie confirms a
    commentary or an audio description whatever the content rule says (D12; menu maps design 5.4a). Every other case
    still follows this decision.`
  - The 8.6 step 1 bullet: `streams a menu map confirms as commentary or audio description (D12; menu maps design
    5.4a). This drop is made after the steps below have arranged the kept tracks, not with the bullets above, and
    only under the conditions of docs/menu-maps/format.md, "The rule": a map is given and its disc ID matches the
    recording's; the title's own playlist was read; the tie is strong; plain Play does not itself select the stream;
    the stream is kept, no Keep override names it, and it is not the programme reference. The content rule's verdict
    is not asked. When the drop would move the reference, or leave the title without audio, nothing is dropped. The
    engine gives no map yet.`
  - The risk "Commentary on Blu-ray and UHD": its sentence of #35 stays as it is.
- [ ] **Step 4: `docs/phase1/decision-core.md`**, the `--menu` paragraph: the decision drops a confirmed stream
  whatever the content rule says of it; the rulings are MR-1 to MR-17 and MA-1 to MA-4.
- [ ] **Step 5: Check** with grep over `docs/` (not `docs/superpowers/plans/`), `crates/` and `README.md` for
  `content rule kept`, `verdict on the stream is keep`, `applied verdict`, `content_not_keep`, `no_verdict`,
  `verdicts_not_applied`, `no_language` and fix every line that is now untrue (records of #35's own acceptance, dated
  2026-11-24, stay as history and say so). Run the whole workspace's tests once.
- [ ] **Step 6: Commit** `docs: the menu's word alone; the owner's rulings after pull request 35`.

### Task 3: acceptance (the controller, outside the repository)

- [ ] Run `accept.py` on the 13 real maps with the new build; list every drop by slug for the owner; fill
  `format.md`'s placeholder with counts only; commit. Then the whole-branch review and the report. Nothing is pushed
  without the owner's word.
