# Menu maps: design

Status: approved by the owner on 2026-11-17. Section 5.4a was added on 2026-11-21 to record the owner's ruling of that
day; its terms beyond the ruling's own words are proposals the owner can correct. Increment 3 (lazymkv reads maps) is
in `main`, and so is the menu-role rule of 5.4a (pull request #35, 2026-11-24; `docs/menu-maps/format.md`, "The
rule"); the owner's answer of that day is at the end of 5.4a. The owner's rulings after that merge widen the rule so
that the menu's word alone drops the track, whatever the content rule says of it; the widening is built on its own
branch (2026-11-24; 5.4a, "The owner's rulings after the merge"). Increment 1 (the tooling in
`experiments/menu-spike/menu/` and the maps of the 13 trial discs) is built. Where it departs from this text the
tooling's README is the description as built: set-up reuses an existing build of the library instead of building it
from pinned source, screenshots are converted without ffmpeg, and `made.profile` records no UHD capability.

## 1. Intent

lazymkv should know, with no person at a television, what each choice on a disc's menu says and what it does: which
playlist "Play" starts, and which audio and subtitle streams each audio, subtitle, commentary and description choice
turns on. That is the link the owner asked for between a user's preferences and the tracks to pull from a disc. A
player knows it because the disc's own menu program tells it; lazymkv should learn it from the same source.

Why now: a trial on 2026-11-17 ran the Java menus of 13 of the owner's 13 Java-menu discs headlessly in a sandbox.
Language-model agents found their way round each menu from screenshots, and an independent check confirmed 107 of 107
examined choices (label read again, replay, stream table). Nothing else on a disc states which stream is which
commentary: the stream table holds only language, codec and channels.

## 2. What the trial established, and what it did not

Established on those 13 discs:
- The open-source Blu-ray library (libbluray 1.5.0) with Java 17 runs each disc's menu program with no screen.
- Only the disc's small files are needed. Video clips can be served as generated blank packets, so nothing large is
  stored. Zero-filled clips do not work: a menu waits on its media clock.
- The disc's own writes to the player registers at film start give the playlist, the audio stream, the subtitle
  stream and whether subtitles are shown. Replays are repeatable to the pixel.
- A macOS sandbox with no network, no writes outside one folder, no reads of the home folder and no other programs
  blocked nothing a disc needed.
- Menus differ: three discs open on a language chooser, most apply a choice only when Play is pressed, five start a
  twin of the playlist that was ripped, and a dubbed audio choice often switches a subtitle stream on.

Not established:
- Any disc outside these 13, any region or language other than region A with English, scene selections, most bonus
  items, combinations of choices, a disc copy with its AACS files present.
- A way for lazymkv to find its way round a menu by itself. The explorers were language models.
- Non-Java discs: three of the owner's 18 test discs have non-Java Blu-ray menus and two are DVDs.

## 3. Goals, non-goals, anti-goals

Goals
1. A **menu map** per disc: a file that lists the disc's menu choices with their labels and effects, each with the
   exact key presses that reproduce it.
2. A map can be **confirmed without any language model** by replaying its key presses and comparing the outcome.
3. The maps supply the menu side of the comparison Stage A never finished: where do menu labels add facts that
   lazymkv's content analysis lacks, and where do the two disagree (kill criteria K5 and K6).
4. lazymkv can **read** a map and show it beside its own decisions, changing none of them (increment 3). Since
   2026-11-21 one narrow case is ruled to change a decision (5.4a); when and where it is built is still the owner's
   choice.
5. Later, lazymkv makes maps **itself**, one menu style at a time, tested against the maps made by agents.

Non-goals for this design
- Running the menu engine inside the Windows engine on the mini PC. Maps are made and replayed on the Mac from
  captured files; the mini PC only reads map files. Moving the engine is a later design.
- A general explorer that works on any unseen menu style. It is the end goal and the code boundary is drawn for it
  (5.5), but styles are added one at a time first.
- A shared or public database of maps.
- Changing any decision rule (R6, D15, D23 and the audio and subtitle rules) on the strength of a label. The owner
  ruled on 2026-11-08 that label against content waits for proof; this design produces that proof and stops there.
  That held until 2026-11-21, when the owner ruled one narrow case (5.4a). Every other case still waits. Where the
  ruled case is built is the owner's choice; 5.4a proposes its own plan after increment 3.

Anti-goals (these would make it a failure even if it works)
- The owner typing or photographing menus as a routine step.
- Disc Java running outside the sandbox, anywhere, ever.
- A map that claims what a track *is*. A map says only: the menu calls this choice X, and choosing it selects stream
  N of playlist P.
- Disc titles, people's names or disc data entering the public repository. Maps hold both and live outside it.
- A silent guess. A choice that could not be confirmed is recorded as unconfirmed, not left out and not rounded up.

## 4. Approaches considered

| Approach | What it is | Trade-off |
|---|---|---|
| A. Built in from the start | lazymkv explores any menu itself: press keys, watch what changes, read labels with its OCR | No language model at any point. The exploring is unproven and the menus differ widely; weeks before the first usable result |
| B. Agent maps only | Keep the trial's method as the product: an agent maps each disc | Works today. Every new disc needs a language model for ever, and lazymkv itself never learns to do it |
| **C. Maps first, built-in after (chosen)** | B as a pipeline now, with replay as the proof; then lazymkv's own navigators, one menu style at a time, tested against the saved maps | Usable maps in days; the built-in work gets an answer key for every style. Mapping a disc of a new style needs an agent until that style has a navigator |

The owner chose C on 2026-11-17. A was not chosen first because its hard part has no evidence yet; B alone leaves the
goal (lazymkv knows by itself) unmet.

## 5. The design

### 5.1 The menu map

One folder per disc under a maps root that is outside every repository (default `$SPIKE_DATA/maps/<slug>/`):
`menu-map.json`, `scripts/` (one key-press script per choice), `img/` (a crop of each label as drawn), and
`replay.json` (the last replay's results).

`menu-map.json`, format name `lazymkv menu map v1`, holds:
- **disc**: slug, the capture's `disc_id_sha1` and `listing_sig`, format (uhd, bd, dvd).
- **made**: who made it (`agent-explorer`, later `navigator/<style>@<version>`), who checked it, the date, the driver's
  hash, the library and Java versions, and the **player profile** used (region, country, menu, audio and subtitle
  language, UHD capability). The profile is an input: a different profile may give a different map.
- **start**: the script that reaches the menu, what had to be passed on the way (a language chooser and the answer
  given), the time it took.
- **play**: what plain Play does with no choice made: playlist, audio stream, subtitle stream and whether shown.
- **choices**: one entry per choice. Each has an id, the section it sits in, the **label** as read, the label image,
  a kind (`audio`, `subtitle`, `version`, `bonus`, `other`), its script, whether it applies at once or at Play, and
  its **effect** (playlist, audio stream, subtitle stream and shown or not). `differs_from_play` says whether the
  effect is different from plain Play; when it is not, the entry shows no effect of choosing and says so.
- **check** per choice: the independent re-reading of the label (same, or the second reading), the number of replays
  and whether they agreed, and a level. The levels are a ladder, per choice: `read` (one reader); `checked` (a second
  reader read the same label and its replay gave the same effect); `replayed` (already `checked`, and the
  deterministic replay agreed three times). A choice whose two label readings differ stays `read` however well its
  key presses replay, because the label is the uncertain part. `player` is a separate mark: the owner saw it on a
  real player. A map's own level is the lowest level among its choices and is only a summary.
- **not_mapped**: what the map leaves out, by name. **quirks**: anything a person would want to know.

What a map covers in this design: the play target and any alternative versions the menu offers; every audio choice;
every subtitle choice; every commentary, description, score and similar track choice, including the pairings needed
to reach commentary subtitles; and, for bonus items, the label and the playlist it starts (no stream detail). Scene
selections are left out.

### 5.2 Making a map (increment 1)

1. **Capture.** The disc's small files, as the spike already captures them (index, MovieObject, PLAYLIST, CLIPINF,
   JAR, BDJO, AUXDATA, META, CERTIFICATE, and a listing with every file's size). Forty discs are captured. A new disc
   is captured on the mini PC in a window the owner opens, with the existing capture tooling.
2. **Run.** The trial's driver and sandbox become repository tooling under `experiments/menu-spike/menu/`: the driver
   source, the session script, the sandbox profile and launcher as templates with the paths filled in at set-up, a
   set-up script that builds the library from its pinned source and checksum, and the how-to. The driver stays C on
   libbluray; this is still experiment tooling, not a lazymkv crate.
3. **Explore.** One agent per disc follows a written procedure kept in the repository: start the disc, pass warnings
   and choosers the way a person would, find the sections, and for each choice record the label, the script and the
   effect. Each choice starts from a fresh session unless the menu makes that impossible.
4. **Check.** A second, independent agent re-reads every label from the screenshot and replays every script. Where
   the two disagree the choice stays at level `read` with both readings.
5. **Replay.** A small program with no language model replays every script through the sandboxed driver and compares
   playlist, audio, subtitle and shown-or-not with the map. A map is `replayed` when every choice agrees three times.
   This is the confirmation lazymkv can run by itself.

The first maps are the 13 trial discs. Twelve have their results in `bdj-trial/trial12-result.json` and
`trial12-verify-result.json` (181 choices, 107 checked); the thirteenth (the first trial disc) has its results in
`bdj-trial/report.md` and `verify.md`, and it is the one disc the owner also walked by hand. Conversion points at
the trial's existing sessions and screenshots and copies the scripts and label crops into the map folders. A
completion run then fills what the trial skipped: the 74 choices not yet checked, the subtitle choices not explored,
and bonus item playlists. All 13 discs are among the 18 that were ripped for Stage A, so each has a rip for the
measurement; that is more than the ten discs K5 needs.

Batch mapping is one command: a workflow script kept in the repository, run by the assistant in a session when the
owner asks for a batch. Approving this design covers the completion run on the 13 trial discs; any later batch runs
on the owner's request. Measured cost in the trial: about 135,000 tokens per disc to explore on a mid-tier model and
about 125,000 to check on the top tier.

The sandbox when it becomes repository tooling: set-up writes the launcher and profile from templates for a run root
it is given (on this Mac the existing `bdj-trial` build and run history are reused, so nothing is downloaded). A
session may read only its own disc folder and the runtime, and write only its own session folder; that is tighter
than the trial, where a session could read every disc copy. On a machine without the build, set-up needs the
library's source: it downloads the pinned tarball and verifies its checksum only when the owner says yes.

### 5.3 The measurement (increment 2)

A converter turns maps into the menu side the existing join already reads (the walk-form shape). How a choice
becomes a row is fixed here, before any content verdict has been joined:

- **Which stream gets the label.** Only the stream of the choice's own kind: an audio choice labels the audio stream
  it selects, a subtitle choice the subtitle stream it shows. A side effect (the subtitle a dubbed audio choice
  switches on) gets no label; the map records it and the results list such pairs apart.
- **The label text** is the section and the choice as printed, joined as `SECTION: label`. On some menus the role is
  only in the section heading (a COMMENTARIES section whose choices are just names), so the heading is part of what
  the menu states. A subtitle choice under a commentary-subtitles heading therefore reads as a commentary subtitle,
  which the join already sets apart as unjudged.
- **Which cell.** Main-menu choices go in the set-up cell, pop-up choices in the pop-up cell, anything else (a bonus
  section) in the other cell.
- **The tie.** A new strong tie word `replay` is used only for a choice at level `replayed` whose effect differs from
  plain Play. A choice whose effect equals plain Play shows no effect of choosing: it gets `attr` when its language
  and codec fit exactly one stream, and otherwise a new weak tie word `default`. A choice below level `replayed` gets
  the weak tie `none` with the reason.
- **Twin playlists.** Where the menu starts a twin of the ripped playlist, streams are matched by comparing the two
  playlists' stream tables (same clip and same stream identifiers); a stream that cannot be matched is reported as
  unjudged with the reason.
- **The hand walk.** The one hand-walked disc keeps its hand-walked form for the measurement. Its map is converted
  too and must give the same labels on the same streams; that comparison is the converter's acceptance test. The
  other 17 hand forms are empty and are not used. The map-derived forms live in their own folder with their own
  freeze.
- **Entries that turn a feature off** are decided in the frozen decisions file the join already requires. The
  assistant writes it from the frozen forms' wording alone, after the freeze and before any content document is
  joined (Rulings T12-18 and T12-19); an entry that names both states of a toggle goes to the owner. Then Stage A's Task 13 runs as designed: lazymkv's content verdicts for the ripped titles (the
release build first, the pinned build as the check), the join, K5 and K6, and the results go to the owner, whose
ruling on label against content was still open when this was written (2026-11-17). The owner ruled one case on
2026-11-21, after reading the results: 5.4a.

### 5.4 lazymkv reads maps (increment 3)

- `core::menu`: types for the map, read with the JSON library lazymkv already uses; tests on made-up maps only.
- `lazymkv-dev menu show <map>` and `lazymkv-dev menu check <map> --scan <scan>`: print a map, and check that it
  belongs to the scanned disc and that every stream it names exists in the scan.
- The decide output gains a `menu` block per title, audio stream and subtitle stream when a map is given: the labels
  that select it and their check level. It is evidence only. No verdict, role or plan changes.

### 5.4a The owner's ruling: a menu role against a content keep (2026-11-21)

**The ruling.** After reading the measurement with their own labels beside it, the owner was asked "Rule that a menu
role with a strong tie outranks a content verdict of keep?" and answered "Yes, rule so". The question put this
recommendation: "Rule narrowly: a menu role with a strong tie outranks a content keep. That is the only kind of
disagreement these discs produced. There is no case here of content positively finding a commentary or audio
description against a different menu role, so that case should stay unruled."

**The evidence.** 14 audio rows on 8 discs, all UHD with Java menus, each with tie `replay` and the content verdict
`keep`: 9 where the menu says commentary, 5 where it says audio description. The owner's labels, made by ear with
neither the menu's wording nor a content verdict shown, name the menu's role on 13, neither side's on 1 and the
content's on 0. Two of the 13 are partial commentaries with the film audible (read as mostly the programme they give
11, 2 and 1), the one "neither" label is marked unsure, and on one row the menu's role rests on an off-decision
(`.superpowers/menu-moonshot/menu-maps-measurement.md`, section 10.1; that folder is git-ignored and is kept in the
spike worktree).

**Its terms.** The owner's words fix a menu role with a strong tie against the content verdict `keep`. They were said
in the measurement's vocabulary, so each term starts from what the measurement meant by it (5.3 and the measurement's
sections 3 and 4). Whatever goes further than that, or narrower, is marked as a proposal the owner can correct.
- **Menu role**: read as the measurement read it (`experiments/menu-spike/walk/map_forms.py` `label_text`,
  `experiments/menu-spike/walk/join.py` `AUDIO_WORDS` and `menu_role`). Each choice's text is the innermost section
  heading, ': ', the label (the label alone when there is no heading; white space collapsed, '|' written '/'). A text
  with no role word is skipped. The one distinct role word among the rest is the stream's role; several different
  ones, or none, give no role. A label part that also says off, none, without or the like gives a role only through a
  recorded decision made from its wording alone, as in 5.3 (an entry that names both states of a toggle goes to the
  owner), never by a guess. On audio the role words give commentary, audio description (`ad`) and programme (the words
  main, programme or program [corrected 2026-11-24: the pattern reads `main`, `programm` and `programme` between word
  edges, not `program`; `docs/menu-maps/format.md`, "Roles"]). The join reads a content keep as programme
  (`AUDIO_ROLE`), so commentary and audio description are the roles that can outrank it.
- **Strong tie**: the measurement's strong ties, unchanged. From a map: `replay` (the choice is at level `replayed`,
  plain Play has a recorded effect that names a playlist, and the choice's playlist or its own kind of stream differs
  from Play's) and `attr` (a choice at level `replayed` that `replay` does not fit, whose text names exactly one
  language, with its codec if it names one, which exactly one stream of that kind has, the one the choice selects).
  From a form typed at a real player: `osd` and `press`; lazymkv reads no such form, so as proposed the rule meets
  only `replay` and `attr`. A stream labelled by several choices has the weakest of their ties. The stream is on the
  title's playlist, or on a twin that shares at least 90% of the menu playlist's duration in play items and is matched
  by stream identifiers. A twin's entry that differs in language or codec from the ripped playlist's entry is
  unmatched and has no tie (`map_forms.py` `match_streams`). Where the ripped playlist's entry differs in language or
  codec from the file track it maps to, the tie is `tie-conflict` (`join.py` `mismatch`), which is not strong; in
  lazymkv the scan stream that decide's verify report pairs with that file track takes the file track's place (a
  proposal). All 14 rows of the evidence have `replay`, none through a playlist difference alone; the owner can limit
  the rule to `replay`.
- **Content verdict of keep**: on audio, the candidate verdict `keep`, the rule's fall-through ("keep: programme
  audio, nothing added", `crates/core/src/audio/rule.rs` on `main`), returned when no other pattern fired; on the 14
  rows the track was kept without being called a commentary or an audio description (measurement 10.1). The
  measurement took a stream's first candidate verdict in decide's audio section, whatever reference it was measured
  against (`join.py` `audio_content`). Two proposals, both narrower than the owner's words. Audio only: a subtitle's
  `keep` means "held for review", the measurement gave it no content role, so a subtitle `keep` against a menu role
  could never show as a disagreement, and content has no commentary role for subtitles; the owner can extend the rule
  to subtitles. And the keep that counts is the one decide applies to the stream: measured against the decision's own
  reference, with no veto, on a kept stream in a wanted language; not `not-judged`, `reference-unclear` or
  `duplicate`, and never the reference track.
- **Outranks** (a proposal; the owner's word does not say what follows): only when a map is given (`lazymkv-dev decide
  --menu`; the engine gives no map until the owner says otherwise). decide's `decision` section gives the stream the
  menu's role, with the menu as its source and the choices that carry it. The port's six sections are unchanged, the
  content verdict as measured (`.audio[].candidates[].verdict`) and the top-level `reviews` included, and without a
  map every document is byte-identical. The decision treats the stream as it treats content's own verdict of that role
  today (D15, RP7): commentary raises `commentary_detected`, so the job goes to review; audio description adds the
  note `possible_audio_description`. Nothing is dropped, moved, renamed or flagged, so R6 and D12 stand. Step 1's flag
  and database drops come first. Remembered and owner overrides decide drops, order and default as they do today, and,
  as with content's verdict, the role still raises its review reason or note on a kept stream that has an override.
  The other reading, dropping the track as a confirmed commentary or audio description, would add the menu to the
  confirmations of R6, D12 and D15; that choice is the owner's.

**Formats (a proposal).** The ruling names no format, and the evidence is UHD with Java menus only. The terms above
treat every map alike, so as proposed the rule would also act on BD, DVD and non-Java maps once such maps exist (a
Blu-ray with Java menus can be mapped by agents today, 5.5; non-Java and DVD menus need increment 5). The owner can
limit it to UHD Java maps until those formats are measured.

**Still unruled.** A menu programme role (the words `main`, `programm` or `programme`) against a content verdict of
commentary or audio description; weak ties (`order`, `ear`, `default`, `none`) and tie conflicts; naming or flagging a
track on a menu role; any user preference that acts on a role. Subtitles are left out by the audio-only proposal above,
not by the ruling; the owner can bring them in. [Amended 2026-11-24, after the merge of pull request #35: the owner's
rulings at the end of this section rule what this paragraph also listed, a menu role against `not-judged`,
`reference-unclear` or a vetoed title, content's commentary or audio-description verdict against a menu role of the same
or the other unwanted kind, and MR-12's track that the menu and the content rule both call a commentary or an audio
description. The reference track is never dropped, as before.]

**Where it is built (a proposal; the owner has not chosen).** Recommended: its own increment, the menu-role increment,
after increment 3 and before the navigators, with its own plan and its own branch from `main`. Increment 3's `menu`
block stays evidence only, carries no role and no tie, and counts a twin playlist's choices under `other_playlists`
(playlist, number of entries and whether the scan lists it), where 4 of the 14 rows sit. The rule's inputs are the map
(through increment 3's `core::menu`); the recording's playlists under `nav/BDMV/PLAYLIST/`, whose stream tables give
each map stream's language, codec and stream identifier for the language and codec check and for twin matching
(lazymkv has no parser for them yet, M3-1); the decisions on label parts that turn a feature off (map v1 has no field
for them: an added optional key, which the v1 reader ignores, or a file beside the map, written as in 5.3); and
decide's content verdicts. A map stream reaches decide's stream as increment 3 ties it (M3-1: by order among the scan
title's streams), after a twin's number is translated to the title's playlist by stream identifier (a proposal); the
measurement went by stream identifier to the ripped file track instead (`join.py`), and the 14-row check below reports
any stream where the two ties differ. The role words and the ties move from the spike's converter and join into
`core::menu`, and the decision core applies the rule. On `main`, D12, D15, D23, RP7, 8.6 (step 6's notes and its
"Content" paragraph), 8.8's table of review reasons, the risk "Commentary on Blu-ray and UHD" and the decide section
of `docs/phase1/decision-core.md` gain a dated sentence for it. Its tests use made-up maps, playlists and verdicts
only (section 9). A check that the 14 rows fire through decide's own verdict selection runs on the existing rips
outside the repository and reports counts and slugs only; the join read every measured folder, so some of the 14 may
not fire as proposed. Whatever the count, it goes to the owner with the reason each missing row did not fire, and the
rule is not changed to make a row fire (a proposal). The alternatives: fold the rule into increment 3 as a last task,
or wait until BD and DVD maps are measured.

**The owner's answer (2026-11-24).** While the menu-role increment was planned, the owner was asked "A menu map says a
track is a commentary or audio description (strong tie) and lazymkv's content check kept it as programme. What should
lazymkv do with that track?" and answered "Drop it as confirmed", over the other choice offered, "Send the job to
review". The "Outranks" proposal's reading above (a review for a commentary, a note for an audio description, nothing
dropped) is thereby replaced by a drop: where a map is given, the menu joins the confirmations of R6, D12 and D15. The
other terms are the rulings MR-2 to MR-16 of the menu-role plan
(`docs/superpowers/plans/2026-11-24-menu-role-increment.md`), proposals the owner can correct; as built they are
written in `docs/menu-maps/format.md`. As built, increment 3's `menu` block also carries the title's `roles` with their
ties (MR-15), so "stays evidence only, carries no role and no tie" above now holds for its other keys only. The final
review added MR-17 (a stream plain Play itself selects is never confirmed), recorded in `docs/menu-maps/format.md`.

**The owner's rulings after the merge (2026-11-24).** After pull request #35 merged, the owner was asked three
questions:
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

The plan that builds them (`docs/superpowers/plans/2026-11-24-menu-alone.md`) reads them as these rulings, proposals
the owner can correct; as built they are written in `docs/menu-maps/format.md`, "The rule":
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

Of MA-4's six sentences, five are replaced; the sixth, in the risk "Commentary on Blu-ray and UHD", is true as it
stands and is kept.

So the menu's word alone drops a track it confirms as a commentary or an audio description, where a map is given,
whatever the content rule says of it and also without a verdict; the guards stand as built (MR-3, MR-4, MR-5, MR-6,
MR-8, MR-9, MR-13, MR-17, and never the programme reference).

### 5.5 Built-in navigators (increment 4)

**One boundary.** Everything that makes a map sits behind one contract: given a disc's captured files, return a menu
map or say plainly that it could not. The rest of lazymkv sees only maps. Behind the contract there is one engine and
several ways of steering it:

- **The engine** holds what is the same on every menu: start a fresh session, press a key and see what changed on
  screen, find the highlighted area, read its label with lazymkv's OCR, apply a choice and watch the registers at
  film start, replay. This is the reusable core of any crawler.
- **A style profile** is the knowledge of one menu style, kept as data where possible and as a small piece of code
  where not: how to get from start-up to the menu, where the sections are, how a section opens, whether a choice
  applies at once or at Play. A navigator is the engine steered by a profile.
- **The agent explorer** of increment 1 is another way of steering the same sessions, behind the same contract.
- **A general explorer**, which works out an unseen menu's layout by itself, would be one more: the same engine
  steered by discovery in place of a profile. It is not built in this design, but nothing in the boundary has to
  change to add it, and the profiles and maps are its test set.

A dispatcher picks by the disc's file fingerprint, which the spike's census already computes: a profile if the style
has one, otherwise the answer "needs mapping", never a guess.

A navigator is accepted when, on every agent-mapped disc of its style, its map has the same choices with the same
effects, and its labels match the agent-read labels after normalising case, spacing and accents; and again on discs
of that style mapped after it was written. Styles are done in order of how many of the owner's discs they cover.
Until a style has a navigator, its discs are mapped by agents. The general explorer is the stated end of this road,
after the styles on the owner's shelf are covered and the engine has been proven on them.

### 5.6 Non-Java discs (increment 5)

Non-Java Blu-ray menus draw their buttons from data inside the menu clips, so blank clips are not enough: those discs
go back in once and their small menu clips are captured through the owner's own decryption path, which Stage A showed
working. The same driver then runs them. DVDs need a second small driver on the DVD navigation library and the menu
files. Both write the same map format.

## 6. Constraints

- Disc Java runs only through the sandbox launcher. The launcher and profile are checked by hash before every run.
- Spike tooling is Python 3.9 standard library and C; no new Rust crates without the owner's yes; Rust builds stay
  `--locked --offline`.
- No disc goes in and the engine is not stopped outside a window the owner opens. On the mini PC only `lazymkv-dev`
  is built, never the workspace.
- Maps, screenshots and logs stay outside the repository. Repository files use slugs, never titles or names.
- Commits are signed with the owner's noreply identity and carry no agent attribution. Nothing is pushed without a
  yes.
- Sub-agent models are chosen per task: mid tier for exploring, top tier for checking and for design.

## 7. Decisions

Made by the owner
- Build the renderer now; stop the hand walk (2026-11-17).
- Disc-supplied Java may run (2026-11-08), in a sandbox with no network and no writes outside one folder
  (2026-11-17).
- Maps first, built-in navigation after (2026-11-17).
- Label against content stays unruled until there is proof (2026-11-08). It held until 2026-11-21; since then the
  one case below is ruled and every other case stays unruled.
- A menu role with a strong tie outranks a content verdict of keep (2026-11-21). Asked "Rule that a menu role with
  a strong tie outranks a content verdict of keep?", the owner answered "Yes, rule so". That is the whole ruling;
  the terms proposed for it are in 5.4a and under the proposals below.
- A menu role that outranks a content keep counts as confirmed: the track is dropped (2026-11-24). Asked "A menu map
  says a track is a commentary or audio description (strong tie) and lazymkv's content check kept it as programme.
  What should lazymkv do with that track?", the owner chose "Drop it as confirmed" over "Send the job to review". It
  replaces the proposal below that outranking drops nothing; the other terms are MR-2 to MR-16 of the menu-role plan,
  proposals the owner can correct (5.4a, "The owner's answer").
- The menu's word alone drops the track (2026-11-24, after the merge of pull request #35). Asked whether the menu's
  word should drop a track the content check also calls a commentary, the owner chose "Yes, drop it"; asked whether it
  should drop a track the content check has no verdict on, "Yes, menu alone"; asked whether to keep the six guards,
  "Keep all six". The questions are quoted in full in 5.4a, "The owner's rulings after the merge"; the plan's reading
  of them, MA-1 to MA-4, replaces MR-2's verdict conditions and MR-12 and is a proposal the owner can correct.

Made here, as proposals the owner can correct
- Maps are made and replayed on the Mac from captured files; the mini PC only reads them.
- The map covers bonus items by label and playlist only, and leaves scene selections out.
- Maps live under `$SPIKE_DATA/maps`, outside every repository, until the owner says whether they belong in the
  private evidence repository.
- Three agreeing replays make a map `replayed`.
- The first three increments are planned and built in order; the menu-role rule of 5.4a, navigators and non-Java
  discs get their own plans.
- One player profile for now: region A, United States, English.
- The measurement method in 5.3: labels attach only to the stream of their own kind, the section heading is part of
  the label, `replay` is strong only when the effect differs from plain Play, and the hand-walked disc keeps its hand
  form.
- A sandboxed session may read only its own disc and write only its own folder.
- Approving this design covers the agent run that completes the 13 trial discs' maps.
- The terms proposed for the 2026-11-21 ruling (5.4a): the measurement's role reading and its strong ties are used
  unchanged; audio only; the keep is the verdict decide applies; outranking gives the stream the menu's role with
  the effect content's own verdict of that role has today, and drops nothing; every map format counts; it is built
  in its own increment after increment 3.

## 8. Risks

- **A style no agent can map.** Thirteen of thirteen is the owner's shelf, not a sample. The pipeline reports a
  stalled disc with where it stopped; it does not guess.
- **A wrong branch that looks right.** A replay proves repeatability, not that a real player does the same. The
  answer is the `player` level: the owner may spot-check a few choices per disc on the real player, never as a
  required step.
- **Disc code.** Escapes from Blu-ray Java have been published for hardware players. The sandbox is the control;
  running maps only for discs the owner owns keeps the exposure small.
- **Drift.** A new library or Java version can change behaviour. Maps record both, and the replay catches a change.
- **Cost.** A quarter of a million tokens per disc until navigators exist.

## 9. Left to the builder

File and function names; the exact JSON schema beyond the fields in 5.1; how the driver and scripts are laid out in
the repository; the converter's code; the replay program's output format; test fixtures (made up, never from a
disc); the workflow script's shape; the order of the two commands in 5.4.
