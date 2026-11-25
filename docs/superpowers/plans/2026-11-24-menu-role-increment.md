# Menu-role increment implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A menu map changes what lazymkv decides: an audio track that the disc's menu calls a commentary or an audio
description, with a strong tie, is dropped as confirmed where the content rule kept it.

**Architecture:** `core::menu` gains three pure modules: a playlist reader (`mpls.rs`), the measurement's role reading
with the off-decisions (`role.rs`) and its ties with twin playlists (`tie.rs`). `tracks::audio::decide_audio` takes
the confirmed roles as one more fact and drops the stream after its usual steps. `lazymkv-dev decide --menu` reads the
playlists and the off-decisions, hands the confirmations to the decision and shows every role in the `menu` key.

**Tech Stack:** Rust 1.100.0, the workspace's crates only (`regex`, `unicode-normalization`, `serde`, `serde_json`
are already dependencies of `lazymkv-core`). No new crate.

**Spec:** `docs/superpowers/specs/2026-11-17-menu-maps-design.md`, section 5.4a (with 5.3 for the measurement's
method). The format is `docs/menu-maps/format.md`.

## Global Constraints

- The ruling (owner, 2026-11-21): "a menu role with a strong tie outranks a content verdict of keep". The owner's
  answer of 2026-11-24: such a role counts as confirmed, so the track is dropped, as a flagged or described commentary
  is today. Only those two are ruled; every other term is a ruling of this plan (MR-2 to MR-16 below), listed for the
  owner to correct.
- Audio only. Without a map every document of every command is byte-identical to `main`. The engine gives no map.
- Tests use made-up maps, playlists, scans and texts only. No disc title, person's name, host, share or user name in
  any committed file; a disc is named by a neutral slug or not at all.
- Builds and tests run on the Mac only:
  `env RUSTUP_AUTO_INSTALL=0 PATH="$HOME/.cargo/bin:$PATH" LAZYMKV_OCRS_MODELS="$HOME/.cargo/lazymkv-ocrs-models" cargo test --workspace --exclude lazymkv-ocr --locked --offline --no-fail-fast`
  (written `CARGO_TEST` below; a single crate is `-p lazymkv-core <filter>` in place of `--workspace --exclude
  lazymkv-ocr`). `Cargo.lock` does not change. Nothing is built on or written to the Windows mini PC.
- Baseline on `main` e94057a: the count the controller records in the ledger before Task 1. No task may lower it or
  add a warning.
- The controller makes every commit (signed, the owner's noreply identity, no agent attribution). A sub-agent stages
  nothing and commits nothing; its "Commit" step is a report of the files it changed.
- Comments and docs follow the code around them: doc comments on every item, lines to 120 columns, plain sentences
  that name the design section or ruling.
- Nothing is pushed and no pull request is opened.

## Rulings of this plan

- MR-1 (owner, 2026-11-24): a commentary or audio-description menu role with a strong tie is a confirmation; the
  track is dropped.
- MR-2: the drop needs the keep that decide applies (design 5.4a): the content verdicts were measured against the
  decision's own reference, D21's veto does not hold, the stream's verdict is `keep`, the stream is kept by step 1 in
  the Original or English group, and it is not the reference. In every other state the decision is what it is today.
- MR-3: a remembered or owner's Keep override wins over the menu's drop, as it wins over a flag or a description.
- MR-4: when dropping the confirmed streams would change the programme reference, none is dropped on that title.
- MR-5: the rule acts only when the map's `disc_id_sha1` matches the recording's disc ID. An identity that is not
  checked shows the roles and confirms nothing.
- MR-6: off-decisions are per choice, in `menu-off.tsv` beside `menu-map.json`. A role-bearing text the net catches
  that has no decision leaves its stream without a role; decide still runs.
- MR-7: lazymkv's net is the measurement's (same words, on the text and on its NFKC lower-cased form).
- MR-8: without the title's own playlist read from the recording (`nav/BDMV/PLAYLIST`), or when its play items carry
  differing stream tables, no stream of the title has a tie.
- MR-9: `tie-conflict` in lazymkv: the playlist's entry against the scan stream that M3-1 names for it differs in
  language (both known) or in codec family; or the playlist lists another number of audio streams than the scan has
  without `CORE_AUDIO` (then every stream of the title).
- MR-10: documents spell the roles `commentary`, `audio_description`, `programme` (the measurement's `ad` is
  `audio_description`).
- MR-11: a menu drop raises no note and no review, as a flag or a description raises none. The decision lists every
  confirmation it was given with what became of it.
- MR-12: a track that the menu and the content rule both call a commentary or an audio description is unruled and
  stays as today (review or note, nothing dropped).
- MR-13: only a playlist name of five ASCII digits is read from disk.
- MR-14: a `duplicate` drop may name a stream that the menu then drops.
- MR-15: the `menu` key's `other_playlists` still counts a twin playlist's choices (M3-2 stands); the new `roles`
  key holds them translated to the title's streams.
- MR-16: acceptance runs on copies of the 13 recordings without their stub `tmdb.json` (with it decide has no plan
  and so no decision), so the plan's identity is unknown there and English is the only wanted language.

## Review Focus

1. A map for another pressing of the disc, or one whose identity cannot be checked: nothing may be dropped (MR-5).
   Test in Task 5.
2. A label that names a role only to switch it off ("COMMENTARY: Off") with no decision recorded: the stream it
   leaves playing must get no role (MR-6). Tests in Tasks 2 and 3.
3. A playlist file that is cut short, is not a playlist, or is missing: no panic, no tie, the reason shown (MR-8).
   Tests in Tasks 1, 3 and 5.
4. The menu naming the programme reference itself, or a stream whose drop would move the reference: nothing dropped
   (MR-2, MR-4). Tests in Task 4.
5. A map whose stream number lands on a scan stream of another language or codec (a wrong order tie): `tie-conflict`,
   nothing dropped (MR-9). Test in Task 3.

---

### Task 1: the playlist reader

**Files:**
- Create: `crates/core/src/menu/mpls.rs`
- Modify: `crates/core/src/menu/mod.rs` (`mod mpls; pub use mpls::*;`)

**Interfaces:**
- Produces:

```rust
/// One play item: its clip's five-character name, and its in and out time in 45 kHz ticks.
pub struct PlayItem { pub clip: String, pub in_time: u32, pub out_time: u32 }
/// One entry of a stream table: the stream's PID, its coding type, its language as the disc writes it ("" if none).
pub struct PlaylistStream { pub pid: u16, pub coding: u8, pub language: String }
/// A parsed playlist: its play items, the first item's primary audio and PG tables (entry n is `[n - 1]`), and
/// whether every play item carries those same two tables.
pub struct Playlist { pub items: Vec<PlayItem>, pub audio: Vec<PlaylistStream>, pub subtitles: Vec<PlaylistStream>, pub uniform: bool }
impl Playlist { pub fn ticks(&self) -> u64 }   // the sum of out_time - in_time, an item with out < in counting 0
pub fn parse_playlist(bytes: &[u8]) -> Result<Playlist, String>
/// Test support (cfg(any(test, feature = "testsupport"))): the bytes of a made-up playlist.
pub struct MadeUpItem { pub clip: &'static str, pub in_time: u32, pub out_time: u32,
                        pub audio: Vec<(u16, u8, &'static str)>, pub subtitles: Vec<(u16, &'static str)> }
pub fn made_up_playlist(items: &[MadeUpItem]) -> Vec<u8>
```

All four structs derive `Clone, Debug, PartialEq, Eq`.

The layout, all integers big-endian (it is the spike's `tools/bd.py` `parse_mpls`, which read every captured disc):

| Where | What |
|---|---|
| bytes 0..4 | `MPLS`, else Err `not a playlist: the file does not start with MPLS` |
| u32 at 8 | `start`, the offset of the PlayList block |
| u16 at `start + 6` | the number of play items; 0 is Err `the playlist has no play item` |
| `start + 10` | the first play item |
| item at `o` | u16 length `len`; `s = o + 2`; clip = 5 bytes at `s`; u16 at `s + 9` has the multi-angle bit `0x10`; in time u32 at `s + 12`, out time u32 at `s + 16`; `p = s + 32`; when multi-angle: `angles` = u8 at `p`, then `p += 2 + 10 * (angles - 1)`; the stream table is at `p`; the next item is at `o + len + 2` |
| stream table at `p` | `q = p + 4`; counts as u8: video `q`, audio `q + 1`, PG `q + 2`, IG `q + 3`, PiP PG `q + 6`; entries start at `q + 12` in the order video, audio, PG then PiP PG; only the first PG-count entries of the PG run are subtitles |
| entry at `e` | u8 length `l`; u8 type at `e + 1`; PID is u16 at `e + 2` (type 1), `e + 4` (type 2), `e + 3` (types 3 and 4), any other type Err `play item <i>: a stream entry of unknown type <t>`; attributes at `a = e + 1 + l`: u8 length `al`, then `al` bytes, the first the coding type; the next entry is at `a + 1 + al` |
| language | 3 bytes after the coding type and one more byte for codings 0x03, 0x04, 0x80 to 0x86, 0xA1, 0xA2 and 0x92; 3 bytes right after the coding type for 0x90 and 0x91; else "" (bytes read with `String::from_utf8_lossy`) |

Any read past the end of `bytes` is Err `the playlist is cut short at byte <offset>` (the first offset it could not
read); an entry whose attribute length is 0 is Err `play item <i>: a stream entry without a coding type`. `<i>`
counts from 0. `uniform` compares each item's audio and subtitle tables with the first's as `(pid, coding, language)`.
No function panics on any input: use checked offset arithmetic.

- [ ] **Step 1: Write the failing tests** in `mpls.rs` (`#[cfg(test)] mod tests`), built on `made_up_playlist`:
  - `a_made_up_playlist_reads_back_its_items_and_tables`: one item `("00001", 1000, 46000)`, audio
    `[(0x1100, 0x83, "eng"), (0x1101, 0x81, "fra")]`, subtitles `[(0x1200, "eng")]`; expect those values, `uniform`
    true, `ticks() == 45000`.
  - `two_items_with_the_same_tables_are_uniform_and_differing_ones_are_not`.
  - `a_multi_angle_item_reads_its_table_after_the_angle_clips` (the builder takes no angles: patch the bytes of a
    built item by hand in the test, or give the builder a private angle count).
  - `a_secondary_stream_entry_reads_its_pid_at_its_own_offset` (types 2 and 3).
  - `text_that_is_not_a_playlist_is_refused`: `b"MPLX0200"` gives the MPLS sentence; an empty slice gives `the
    playlist is cut short at byte 0`.
  - `every_truncation_of_a_playlist_is_refused_without_a_panic`: for each `n` in `0..bytes.len()` of a two-item
    playlist, `parse_playlist(&bytes[..n])` is Err.
  - `a_playlist_without_play_items_is_refused`, `an_unknown_stream_entry_type_is_refused_naming_its_item`.
  - `an_item_with_its_out_time_before_its_in_time_counts_no_ticks`.
- [ ] **Step 2: Run** `CARGO_TEST -p lazymkv-core menu::mpls` — fails to compile (no `parse_playlist`).
- [ ] **Step 3: Implement** `parse_playlist`, `Playlist::ticks` and `made_up_playlist` (the builder writes exactly
  the layout above: header `MPLS0200`, `start` 40, type 1 entries, attribute length 5).
- [ ] **Step 4: Run** the same command — all pass; then `CARGO_TEST` whole workspace — no failure, no warning.
- [ ] **Step 5: Commit** `menu: read a playlist's play items and stream tables`.

### Task 2: role words and off-decisions

**Files:**
- Create: `crates/core/src/menu/role.rs`
- Modify: `crates/core/src/menu/mod.rs` (`mod role; pub use role::*;`, and
  `pub const ROLE_CASES: &str = include_str!("fixtures/menu-role-cases.json");` under
  `cfg(any(test, feature = "testsupport"))`)
- Test data (already in the tree, generated by the controller from the measurement's own Python over made-up
  texts; do not edit it): `crates/core/src/menu/fixtures/menu-role-cases.json`, keys `label_text`
  (`section`, `label`, `text`), `roles` (`text`, `roles`, `needs_decision`), `names` (Task 3).

**Interfaces:**
- Produces:

```rust
/// What a menu calls an audio stream (design 5.4a): serde and `as_str` give "commentary", "audio_description",
/// "programme" (MR-10). Derives Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize.
pub enum AudioRole { Commentary, AudioDescription, Programme }
/// The measurement's text of a choice: the innermost heading of `section` (after the last " > ", trimmed), ": ",
/// the label; the label alone when that heading is empty; every '|' written '/'; white space runs collapsed to one
/// space; trimmed.
pub fn role_text(section: &str, label: &str) -> String
/// The roles whose words `text` holds, in the enum's order, each once.
pub fn audio_roles_in(text: &str) -> Vec<AudioRole>
/// True when `text` holds a role word and the net catches it or its NFKC lower-cased form (MR-7).
pub fn needs_decision(text: &str) -> bool
pub const OFF_FILE_NAME: &str = "menu-off.tsv";
pub enum OffDecision { Off, Role }
/// The decisions of one map's `menu-off.tsv`, by choice id. `Default` is no decision.
pub struct OffDecisions { /* private: BTreeMap<String, OffDecision> */ }
impl OffDecisions { pub fn get(&self, id: &str) -> Option<OffDecision>; pub fn ids(&self) -> Vec<&str> }
pub fn parse_off(text: &str) -> Result<OffDecisions, Vec<String>>
/// What the choices that label one stream say it is.
pub enum RoleReading { Role(AudioRole), Silent, Several(Vec<AudioRole>), Undecided(Vec<String>) }
/// `choices` are (choice id, `role_text`) in map order.
pub fn stream_role(choices: &[(&str, &str)], off: &OffDecisions) -> RoleReading
```

The words, each matched without regard to case; "word edge" means the neighbouring character is neither
`char::is_alphanumeric()` nor `_`, or there is none (Python's `\b`; do not use the regex crate's `\b`, whose word
characters include combining marks, which the shared case `Programme` + U+0301 pins):

| Role | Matches |
|---|---|
| commentary | `commentar` anywhere |
| audio_description | `audio descri` or `audio-descri` anywhere; `descriptive` anywhere; `described`, `dvs`, `ad` between word edges; `visually impaired` or `visually-impaired` anywhere |
| programme | `main`, `programm`, `programme` between word edges (so `Program` alone is no role word) |

The net, between word edges: `off`, `no`, `none`, `not`, `without`, `disable`, `disabled`, `stop`, `hide`, `hidden`.
`needs_decision` tries the text, then the text after NFKC, `to_lowercase` and white-space collapse.

`menu-off.tsv` (MR-6): UTF-8, a leading byte-order mark dropped, lines split on `\n` with a trailing `\r` left off.
A blank line and a line starting with `#` are skipped. Every other line is `<decision><TAB><choice id>` with the
decision `off` (the choice states no role for the stream it leaves playing: its words are not read) or `role` (its
words are read as any other's). Every problem is listed, one sentence each, in line order:
`line <n>: expected "off<TAB><choice id>" or "role<TAB><choice id>"`;
`line <n>: unknown decision <repr> (use "off" or "role")`;
`line <n>: <repr> is not a choice id (c and three digits)`;
`line <n>: <id> is decided <a> here and <b> on line <m>`. A line repeating an earlier one is dropped. `<repr>` is
`crate::recording::py_repr_str`.

`stream_role`: a text with no role word is skipped. A text with a decision `off` is skipped; with `role` it is read.
A text without a decision that `needs_decision` is undecided: its id is collected and its words are not read. Any
undecided id gives `Undecided(ids)` whatever the other texts say (MR-6). Otherwise the distinct roles read: one is
`Role`, none `Silent`, more `Several` (enum order).

- [ ] **Step 1: Write the failing tests** in `role.rs`:
  - `the_shared_cases_give_the_measurements_label_text`: every `label_text` case: `role_text(section, label) == text`.
  - `the_shared_cases_give_the_measurements_roles_and_net`: every `roles` case: `audio_roles_in(text)` as strings
    equals `roles`, and `needs_decision(text) == needs_decision`.
  - `a_role_spells_as_serde_writes_it`: the three spellings through `as_str` and serde both ways.
  - `an_off_file_reads_decisions_comments_and_a_byte_order_mark`: `"\u{feff}# why\r\noff\tc012\r\n\nrole\tc013\n"`
    gives `c012` Off, `c013` Role, `ids() == ["c012", "c013"]`.
  - `every_problem_of_an_off_file_is_listed`: `"off c012\nmaybe\tc012\noff\tc12\noff\tc001\nrole\tc001\noff\tc001\n"`
    gives the four sentences for lines 1, 2, 3 and 5 (line 6 repeats line 4 and is dropped).
  - `one_role_is_the_streams_role`, `texts_without_role_words_are_silent`,
    `two_different_roles_are_several_in_enum_order`.
  - `an_off_decision_skips_the_words_and_a_role_decision_reads_them`: `[("c001", "COMMENTARY: Off")]` with `off`
    gives `Silent`, with `role` gives `Role(Commentary)`.
  - `an_undecided_text_leaves_the_stream_without_a_role`: `[("c001", "AUDIO: Commentary by Person A"), ("c002",
    "COMMENTARY: Off")]` with no decisions gives `Undecided(vec!["c002"])`.
- [ ] **Step 2: Run** `CARGO_TEST -p lazymkv-core menu::role` — fails to compile.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run** it — pass; then the whole workspace.
- [ ] **Step 5: Commit** `menu: read a choice's role words and the off-decisions`.

### Task 3: ties and the roles of a title

**Files:**
- Create: `crates/core/src/menu/tie.rs`
- Modify: `crates/core/src/menu/mod.rs` (`mod tie; pub use tie::*;`); `crates/core/src/menu/scan.rs` (make
  `playlist_of` `pub(super)`; `MenuBlock` gains a last field `#[serde(default, skip_serializing_if =
  "Option::is_none")] pub roles: Option<RolesBlock>`, which `title_block` sets to `None`, so every block printed
  today is unchanged)

**Interfaces:**
- Consumes: `Playlist`, `PlaylistStream` (Task 1); `role_text`, `stream_role`, `RoleReading`, `AudioRole`,
  `OffDecisions`, `audio_roles_in` (Task 2); `map_streams`, `playlist_of`, `Identity` (`scan.rs`);
  `crate::lang::norm_lang`; `crate::codec::Family`.
- Produces:

```rust
/// How firmly a choice is tied to its stream (design 5.3, 5.4a). serde and `word()` give "replay", "attr",
/// "default", "none", "tie-conflict".
pub enum Tie { Replay, Attr, Default, None, TieConflict }
impl Tie { pub fn is_strong(self) -> bool }          // Replay and Attr
/// The ISO 639-2/T codes `text` names, sorted (the measurement's closed table).
pub fn languages_named(text: &str) -> Vec<&'static str>
/// The codec names `text` names, sorted: Some(empty) when none is named, None when the named ones conflict.
pub fn codecs_named(text: &str) -> Option<Vec<&'static str>>
/// The measurement's name of a coding type: 0x80 "LPCM", 0x81 "AC3", 0x82 "DTS", 0x83 "TrueHD", 0x84 "EAC3",
/// 0x85 "DTS-HD HR", 0x86 "DTS-HD MA", 0xA1 "EAC3", 0xA2 "DTS-HD HR"; None for any other.
pub fn coding_name(coding: u8) -> Option<&'static str>
/// One scan audio stream that at least one audio choice labels.
pub struct StreamRole { pub stream: u32, pub role: Option<AudioRole>, pub tie: Tie, pub choices: Vec<String>, pub why: String }
impl StreamRole { pub fn confirms(&self) -> Option<AudioRole> }  // Commentary or AudioDescription with a strong tie
/// An audio choice that labels no stream of the title, and why.
pub struct Unplaced { pub id: String, pub why: String }
pub struct RolesReport { pub untied: Option<String>, pub audio: Vec<StreamRole>, pub unplaced: Vec<Unplaced>, pub off_unused: Vec<String> }
pub fn audio_roles(map: &MenuMap, off: &OffDecisions, title: &Title,
                   playlists: &BTreeMap<String, Result<Playlist, String>>) -> RolesReport
/// The `roles` key of decide's menu block.
pub struct RolesBlock { pub acts: bool, pub why: String, pub audio: Vec<StreamRole>, pub unplaced: Vec<Unplaced>, pub off_unused: Vec<String> }
impl RolesReport { pub fn block(self, identity: &Identity) -> RolesBlock }
```

`Tie`, `StreamRole`, `Unplaced`, `RolesBlock` derive `Clone, Debug, PartialEq, Serialize, Deserialize` (`Tie` also
`Copy, Eq`). `playlists` is keyed by the playlist's name without `.mpls`, as an effect and `playlist_of` spell it.

`languages_named` and `codecs_named` port `map_forms.py` exactly; the shared `names` cases pin them. Both work on
the text after NFKD, with every character of a canonical combining class other than 0 removed, lower-cased; a phrase
matches only where the character before and after is not `char::is_alphanumeric()` (an underscore is an edge here);
phrases are tried longest first (equal lengths in alphabetical order) and every match of a found phrase is blanked to
a space before the next phrase is tried.

```rust
const LANGUAGES: [(&str, &str); 24] = [
    ("eng", "english"), ("fra", "french francais canadien quebecois"), ("spa", "spanish espanol castellano latino"),
    ("deu", "german deutsch"), ("ita", "italian italiano"), ("por", "portuguese portugues brasileiro"),
    ("jpn", "japanese"), ("kor", "korean"), ("zho", "chinese mandarin cantonese"), ("nld", "dutch nederlands"),
    ("rus", "russian"), ("pol", "polish polski"), ("ces", "czech cesky cestina"), ("hun", "hungarian magyar"),
    ("tha", "thai"), ("tur", "turkish turkce"), ("swe", "swedish svenska"), ("dan", "danish dansk"),
    ("nor", "norwegian norsk"), ("fin", "finnish suomi"), ("ell", "greek"), ("heb", "hebrew"), ("ara", "arabic"),
    ("hin", "hindi"),
];
const CODEC_PHRASES: [(&str, &[&str]); 16] = [
    ("dolby digital plus", &["EAC3"]), ("dd+", &["EAC3"]), ("e-ac-3", &["EAC3"]),
    ("dolby digital", &["AC3"]), ("ac-3", &["AC3"]), ("ac3", &["AC3"]),
    ("truehd", &["TrueHD"]), ("atmos", &["TrueHD", "EAC3"]),
    ("dts-hd master audio", &["DTS-HD MA"]), ("dts-hd ma", &["DTS-HD MA"]), ("dts-hd master", &["DTS-HD MA"]),
    ("dts:x", &["DTS-HD MA"]), ("dts-hd", &["DTS-HD HR", "DTS-HD MA"]), ("dts", &["DTS", "DTS-HD HR", "DTS-HD MA"]),
    ("lpcm", &["LPCM"]), ("pcm", &["LPCM"]),
];
// codecs_named is the intersection of the found phrases' sets. Each word of a LANGUAGES row is its own phrase.
```

`audio_roles`, in this order:

1. `off_unused`: every id of `off` that names no audio choice of the map, or one whose `role_text` holds no role
   word; id order. It does not depend on the title.
2. The block is `playlist_of(title.source_file)`. `untied` is Some, and `audio` empty, when: the title has no source
   file (`the title has no playlist`); `playlists` lacks the block (`playlist <p> was not read`); its entry is Err
   (`playlist <p>: <error>`); it is not `uniform` (`playlist <p>: its play items carry differing stream tables`).
   `unplaced` is then empty too.
3. Each choice of kind `Audio`, in map order (other kinds are passed over):
   - no effect: unplaced `no effect`;
   - effect on the block (`eq_ignore_ascii_case`): its own number is the block number;
   - effect on another playlist that `playlists` holds as Ok and uniform, whose play items shared with the block
     (same clip, in and out time, each block item counted once) cover at least 9/10 of the menu playlist's `ticks()`
     (which must be above 0): a twin. The menu playlist's audio entry `n` is translated to the block entry with the
     same PID: none is unplaced `unmatched: pid not in <block> (menu <p> audio <n>)`; several `unmatched: pid twice
     in <block> (menu <p> audio <n>)`; one whose `norm_lang` or coding differs `unmatched: attributes differ in
     <block> (menu <p> audio <n>)`; a number the menu playlist does not hold `unmatched: no audio <n> in the menu
     playlist`;
   - any other playlist: unplaced `other playlist <p>` (`playlist_text`);
   - `effect.audio` null: unplaced `no stream number`; a block number below 1 or above the block's audio count:
     unplaced `unmatched: no audio <n> in <block>`; a block number above the scan's count (`map_streams(title)
     .audio`): unplaced `no scan stream for audio <n>`.
   - Otherwise the choice labels scan stream `map_streams(title).audio[n - 1]` with this tie: below level `Replayed`
     `None` (`level <word>`); `Replay` when plain Play has an effect whose playlist is not empty and the choice's
     playlist or audio number differs from Play's (compared exactly, the menu's own numbers); else `Attr` when
     `languages_named(text)` is one language, `codecs_named(text)` is not None, exactly one block audio entry has
     that `norm_lang` and (when a codec is named) a `coding_name` among those named, and that entry is the choice's
     block number; else `Default` with the measurement's reason (`attr: <k> languages named`, `attr: codecs conflict`,
     `attr: <k> streams fit`, `attr: names another stream`).
4. Per labelled stream, ascending: `choices` in map order; `tie` the weakest of its choices' ties (`Replay`, `Attr`,
   `Default`, `None` from strong to weak); `role` from `stream_role` over `(id, role_text)`. Then MR-9: when the
   block lists another number of audio streams than `map_streams(title).audio` holds, or the block entry and the scan
   stream differ in language (`norm_lang` of both, neither among `crate::lang::NO_LANGUAGE`), or the scan stream's
   `codec.family` is not one the entry's coding allows (0x80 Lpcm; 0x81 Ac3; 0x82 Dts; 0x83 TrueHd; 0x84 Eac3; 0x85
   DtsHdHr or Dts; 0x86 DtsHdMa or Dts; any other coding allows every family), the tie is `TieConflict`.
5. `why` is empty when the role is Some and the tie strong. Otherwise the first that applies: the conflict (`the
   playlist lists <a> audio streams, the scan <b>`; `language <a> vs <b>`; `codec <coding as 0xNN> vs <the scan
   codec's display>`); `no role word`; `several roles: <words joined with ", ">`; `undecided: <ids joined with ", ">
   (menu-off.tsv)`; `tie <word>: <the weakest choice's reason>`.

`block`: `acts` is true when `untied` is None and `identity` is `Matches`; `why` is `untied`'s reason, else `the
map's identity is <Identity::text()>` when it is not `Matches`, else empty.

- [ ] **Step 1: Write the failing tests** in `tie.rs`. A test map is built in code (a helper that returns a
  `MenuMap` with given play effect and choices) and playlists with `made_up_playlist`; the scan title with
  `crate::disc` types as `menu/tests.rs` builds its titles.
  - `the_shared_cases_give_the_measurements_languages_and_codecs`: every `names` case.
  - `a_replayed_choice_that_differs_from_play_has_the_replay_tie`: Play on `00800` audio 1; choice `c010`
    "AUDIO: Commentary by Person A" on `00800` audio 3 at level replayed; the title has three audio streams; expect
    one `StreamRole` with `role == Some(Commentary)`, `tie == Replay`, `choices == ["c010"]`, `why == ""`,
    `confirms() == Some(Commentary)`.
  - `a_choice_below_replayed_has_no_tie` (`tie None`, why `tie none: level checked`, `confirms()` None).
  - `a_choice_equal_to_play_is_attr_when_its_language_and_codec_fit_one_stream_and_default_otherwise` (four cases:
    attr; two languages named; two streams fit; names another stream).
  - `without_a_play_effect_there_is_no_replay_tie`.
  - `several_choices_on_one_stream_have_their_weakest_tie`.
  - `a_twin_playlists_choice_is_translated_by_pid`: Play and the choice on `00801`, the title is `00800`; both
    playlists share one item; `00801` lists the commentary as audio 2 with PID 0x1102, `00800` as audio 3; expect the
    role on the scan stream of block audio 3.
  - `a_twin_entry_that_differs_in_language_or_codec_is_unmatched`, `a_pid_the_block_lacks_is_unmatched`.
  - `a_playlist_sharing_under_nine_tenths_is_another_playlist` (unplaced `other playlist 00900`).
  - `a_wrong_order_tie_is_a_tie_conflict`: the block's audio 3 is `fra`, the scan's third audio stream `eng`:
    `tie == TieConflict`, why `language fra vs eng`, `confirms()` None. Also the codec case and the count case.
  - `without_the_titles_playlist_no_stream_is_tied`: the four `untied` reasons.
  - `an_undecided_off_text_gives_no_role_and_a_decision_settles_it` (through `audio_roles`, with `off` and `role`).
  - `a_programme_role_confirms_nothing`.
  - `unused_decisions_are_listed` (an unknown id, a subtitle choice's id, an audio choice without a role word).
  - `the_block_acts_only_on_a_matching_identity` (`Matches`, `NotChecked`, and an `untied` report).
- [ ] **Step 2: Run** `CARGO_TEST -p lazymkv-core menu::tie` — fails to compile.
- [ ] **Step 3: Implement** the items above.
- [ ] **Step 4: Run** it — pass; then the whole workspace.
- [ ] **Step 5: Commit** `menu: tie a map's audio choices to scan streams and read their roles`.

### Task 4: the rule in the decision core

**Files:**
- Modify: `crates/core/src/tracks/audio.rs`
- Test: `crates/core/src/tracks/audio/tests.rs`

**Interfaces:**
- Consumes: `crate::menu::AudioRole` (Task 2).
- Produces:

```rust
/// A menu map's confirmation of one scan stream (design 5.4a, MR-1): the role and the choices that carry it.
pub struct MenuConfirmation { pub role: AudioRole, pub choices: Vec<String> }     // Clone, Debug, PartialEq
// AudioFacts gains:  pub menu: BTreeMap<u32, MenuConfirmation>,   (empty by default)
// AudioDrop gains, last:  Menu { role: AudioRole },                written {"why":"menu","role":"commentary"}
/// What became of a confirmation; serde snake_case.
pub enum MenuOutcome { Dropped, NotKept, Overridden, NoLanguage, Reference, NoVerdict, VerdictsNotApplied, ContentNotKeep, ReferenceWouldChange }
pub struct MenuApplied { pub stream: u32, pub role: AudioRole, pub choices: Vec<String>, pub outcome: MenuOutcome,
                         #[serde(default, skip_serializing_if = "Option::is_none")] pub verdict: Option<Verdict> }
// AudioDecision gains, last:  #[serde(default, skip_serializing_if = "Vec::is_empty")] pub menu: Vec<MenuApplied>,
```

The rule, in `decide_audio`, after today's steps have given the first arrangement (kept tracks, reference, notes,
reviews). A confirmation whose role is `Programme` is ignored and not listed. For each other confirmation, ascending
stream, the first that applies is its outcome:

1. `NotKept`: the stream is not among the kept tracks (step 1 dropped it, or the title has no such audio stream).
2. `Overridden`: it has a remembered or owner's Keep override (MR-3).
3. `NoLanguage`: it is kept in the `NoLanguage` group.
4. `Reference`: it is the decision's reference.
5. `NoVerdict`: `facts.verdict_reference` is None.
6. `VerdictsNotApplied`: the decision has no reference, the reference is not the measured one, or `veto_holds`.
7. `NoVerdict` when `facts.verdicts` has no entry for it; `ContentNotKeep` with `verdict` when the entry is not
   `Verdict::Keep`.
8. Otherwise it is a candidate.

With at least one candidate, the kept streams without the candidates are arranged again by the same code (rank,
guards, order, reference, flags, override order and default, notes, gaps, verdicts). When that reference equals the
first one, the second arrangement is the decision, each candidate is added to `dropped` as `AudioDrop::Menu { role }`
(`dropped` stays in scan order) with outcome `Dropped`. When it differs, the first arrangement stands and every
candidate's outcome is `ReferenceWouldChange` (MR-4). `verdict` is Some only with `ContentNotKeep`. Confidence, D13's
duplicates and step 1 are untouched (MR-14). With `facts.menu` empty the function returns exactly what it returns
today and `menu` is empty.

- [ ] **Step 1: Write the failing tests** in `tracks/audio/tests.rs`, with the file's own helpers (`title_of`,
  `video`, `truehd`, `dd`, `decide`-like calls that pass facts). The base title: video, `truehd("eng", "8")` (stream
  1), `dd("eng", "2", "192")` (2), `dd("eng", "2", "192")` (3); facts `verdict_reference: Some(1)`, verdicts `{2:
  Keep, 3: Keep}`, menu `{2: Commentary ["c010"], 3: AudioDescription ["c011"]}`.
  - `a_menu_commentary_and_a_menu_audio_description_are_dropped_where_content_kept_them`: kept `[1]`; `dropped ==
    [(2, Menu { role: Commentary }), (3, Menu { role: AudioDescription })]`; no note names stream 2 or 3; `reviews`
    empty; `confidence == AUDIO_CONFIDENCE`; `menu` has both with `Dropped`; stream 1 is default.
  - `a_menu_drop_is_written_with_its_why_and_role`: the JSON `{"why":"menu","role":"audio_description"}` and a
    `MenuApplied` as `{"stream":2,"role":"commentary","choices":["c010"],"outcome":"dropped"}`.
  - `without_confirmations_the_decision_is_todays`: the same facts with `menu` empty equal `decide_audio` of main's
    behaviour: kept `[1, 2, 3]`, the two `possible_commentary` notes, `menu` empty, and its JSON has no `menu` key.
  - `a_content_commentary_verdict_stays_a_review_and_is_not_dropped` (verdict `Commentary` on 2: `ContentNotKeep`
    with `verdict: Some(Commentary)`, `commentary_detected` raised, stream 2 kept) (MR-12).
  - `the_other_verdicts_drop_nothing`: `NotJudged`, `ReferenceUnclear`, `AdPattern`, `NoteCommentary`, `NoteAd`,
    `Duplicate` each give `ContentNotKeep`.
  - `no_measured_reference_or_no_verdict_drops_nothing` (`NoVerdict` twice: `verdict_reference` None; no entry).
  - `a_veto_or_another_reference_drops_nothing` (`VerdictsNotApplied`; `audio_reference_unclear` as today).
  - `the_reference_is_never_dropped` (menu names stream 1: `Reference`).
  - `a_keep_override_wins_over_the_menu` (remembered and reviewed: `Overridden`, the stream kept).
  - `a_stream_step_1_dropped_is_not_kept` (a flagged stream; an unwanted language) and
    `a_no_language_stream_is_not_dropped` (`NoLanguage`).
  - `a_programme_confirmation_is_ignored` (not listed, nothing dropped).
  - `a_drop_that_would_move_the_reference_is_not_made`: build a title where removing the confirmed stream changes
    `reference` (guard G2: the confirmed stream is the first of its language; find the smallest such title with the
    file's helpers and say in the test's comment why the reference moves); expect the first arrangement and
    `ReferenceWouldChange`.
  - `dropping_a_stream_recomputes_first_of_language_and_the_notes`: a title where the dropped stream was the first
    of its language; the remaining tracks' `first_of_language` and notes are those of the title without it.
- [ ] **Step 2: Run** `CARGO_TEST -p lazymkv-core tracks::audio` — fails to compile.
- [ ] **Step 3: Implement.** Split the arrangement out of `decide_audio` into a private function that both passes
  call; update the doc comments of `decide_audio`, `AudioDrop`, `AudioFacts` and `AudioDecision` to say what the
  menu does and name design 5.4a and MR-1 to MR-4.
- [ ] **Step 4: Run** it — pass; then the whole workspace (every `AudioFacts { .. }` literal and every match on
  `AudioDrop` in the workspace compiles; the engine and the clients show an unknown drop reason as they show the
  others, add the arm where a match is exhaustive).
- [ ] **Step 5: Commit** `audio: drop a track the menu confirms as commentary or audio description`.

### Task 5: `decide --menu` hands the roles to the decision

**Files:**
- Modify: `crates/dev/src/cmd/menu.rs` (readers), `crates/dev/src/cmd/decide.rs` (inputs, decision, menu key, help
  text, module docs)

**Interfaces:**
- Consumes: `audio_roles`, `RolesReport::block`, `RolesBlock`, `MenuBlock.roles`, `StreamRole::confirms` (Task 3); `parse_off`,
  `OffDecisions`, `OFF_FILE_NAME` (Task 2); `parse_playlist`, `Playlist`, `made_up_playlist` (Task 1);
  `MenuConfirmation` (Task 4).
- Produces:

```rust
// cmd::menu:
/// The off-decisions beside the map `given` names (`<folder>/menu-off.tsv`; for a map file, its folder's): none
/// when the file is absent; Refused { code: 2 } with one line `<shown>: <problem>` per problem otherwise.
pub(crate) fn read_off(ctx: &Ctx, given: &str) -> Result<OffDecisions, Refused>
/// Each named playlist of the recording, `nav/BDMV/PLAYLIST/<name>.mpls`: parsed, or why not (`not a playlist
/// name` for a name that is not five ASCII digits, which is not looked up (MR-13); `no such file`; the read error;
/// `parse_playlist`'s sentence).
pub(crate) fn read_playlists(rec: &Recording, names: &BTreeSet<String>) -> BTreeMap<String, Result<Playlist, String>>
// cmd::decide::MenuInput gains:  pub off: OffDecisions,  pub playlists: BTreeMap<String, Result<Playlist, String>>,
// cmd::decide::FactsSummary gains, last:
//   #[serde(default, skip_serializing_if = "Vec::is_empty")] pub menu_streams: Vec<u32>,
```

Behaviour:
- `run`: after the map is read, `read_off` (a refusal prints its lines as the map's refusal does and exits 2). After
  `read_decide_inputs`, the names to read are the label's title's playlist (from the scan, when it can be typed and
  lists the title) and the playlist of Play's effect and of every audio choice's effect; `read_playlists` fills
  `MenuInput.playlists`.
- One private function gives the title's `RolesReport` and `Identity` from a `MenuInput` and the inputs, or None
  where `menu_section` is `Skipped` today (meta.json unreadable, the map is for another disc, the scan cannot be
  typed, the scan lacks the title). Both users call it, so the decision and the shown block cannot disagree.
- `decision_section`: when the block `acts`, `AudioFacts.menu` holds `MenuConfirmation { role, choices }` for every
  `StreamRole` whose `confirms()` is Some, and `FactsSummary.menu_streams` lists those streams; otherwise both are
  empty. The roles are read against the scan's title (`scanned`), not the narrowed rip title.
- `menu_section`: the block's `roles` is `Some(report.block(&identity))`.
- Help text: `--menu MAP` reads `a menu map, its folder or its menu-map.json: adds the menu key, and a commentary or
  audio description it confirms is dropped (docs/menu-maps/format.md)`. The module docs and the `MenuEntry`,
  `MenuInput` and `DecideDoc.menu` comments stop saying "evidence only" and say what changes: the `decision` key and
  nothing else.
- The engine and `read_decide_inputs` still give `menu: None`.

- [ ] **Step 1: Write the failing tests.** In `cmd/menu.rs`: `read_off_reads_the_file_beside_the_map` (folder form,
  file form, absent file is no decisions, a malformed file is refused with `<shown>: line 1: ...` lines) and
  `read_playlists_reads_only_five_digit_names` (a made-up playlist is read; `../x` and `0080` give `not a playlist
  name`; a missing file gives `no such file`; a cut file gives the parser's sentence). In `cmd/decide.rs`, on the
  made-up recording whose decision has content verdicts (the one the tests around `verdict_streams` use, where
  `facts.verdict_streams` is `[2, 3, 4]` against reference 1; pick a stream there whose verdict is `keep`, or set
  one up as those tests do), with a made-up map (the fixture's shape, built in the test), `meta.json` naming the
  map's ID, and made-up playlists under `nav/BDMV/PLAYLIST`:
  - `a_confirmed_role_drops_the_stream_and_changes_only_the_decision_and_menu_keys`: the document with the map
    equals the one without in `slug`, `label`, `reconcile`, `audio`, `subtitles`, `reviews` and `plan` (compared as
    printed text key by key); `decision.decision.audio.dropped` holds `{"why":"menu","role":"commentary"}` for the
    stream; `decision.decision.audio.menu[0].outcome == "dropped"`; `decision.facts.menu_streams == [<stream>]`;
    `menu.roles.acts == true` and its row has `tie: "replay"`.
  - `a_map_whose_roles_confirm_nothing_changes_only_the_menu_key` (replaces
    `a_map_adds_only_the_menu_key_and_changes_no_decision`: the shared fixture map, whose choices carry no role word).
  - `an_identity_that_is_not_checked_confirms_nothing` (no `meta.json`: `roles.acts == false`, `why` names the
    identity, the decision equals the one without a map) and `a_map_for_another_disc_is_skipped_and_drops_nothing`.
  - `without_the_recordings_playlist_nothing_is_confirmed` (no `nav` folder: `roles.acts == false`, `roles.why ==
    "playlist 00800: no such file"`, the decision equals the one without a map).
  - `a_malformed_off_file_exits_2_with_its_lines` and `an_off_decision_reaches_the_roles`.
  - `without_a_map_there_is_no_menu_key` still passes unchanged, and every characterisation document is unchanged.
- [ ] **Step 2: Run** `CARGO_TEST -p lazymkv-dev menu` and `... -p lazymkv-dev decide` — fail to compile.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** them — pass; then the whole workspace: no failure, no warning, no characterisation diff.
- [ ] **Step 5: Commit** `decide: a menu map's confirmed roles reach the decision`.

### Task 6: the documents

**Files:**
- Modify: `docs/menu-maps/format.md`, `docs/superpowers/specs/2026-11-17-menu-maps-design.md`,
  `docs/superpowers/specs/2026-10-04-lazymkv-design.md`, `docs/phase1/decision-core.md`, `README.md` (only if it
  says maps are evidence only)

**Interfaces:**
- Consumes: the names and sentences of Tasks 1 to 5 as built (read the code, not this plan, for the final wording).

- [ ] **Step 1: `format.md`.** The opening paragraph names the design by its path. "A map changes no decision" and
  "How decide shows a map" say what is true now. New sections, each with a made-up example: **Roles** (the text, the
  three roles and their words, the net); **The off-decisions file** (`menu-off.tsv`, its lines and its refusals);
  **Playlists** (what is read, from where, MR-8, MR-13; M3-1's "parses no playlist" becomes "the order tie is still
  M3-1's; the playlist checks it"); **Ties** (the five words, twins, MR-9); **The rule** (MR-1 to MR-4, MR-11,
  MR-12, the nine outcomes); the `roles` key and the two new keys of `decision`; under **Rulings**, MR-1 to MR-16
  verbatim from this plan with the date 2026-11-24; **Known reader differences** gains the two the shared cases do
  not reach (Python's case-insensitive match treats a long s as an s; white space is Unicode's `White_Space`).
  "Tests and acceptance" keeps its 2026-11-21 record and gains a dated paragraph the controller fills in Task 7.
- [ ] **Step 2: the menu-maps design.** Status line: the menu-role increment is built on its branch. 5.4a gains a
  last paragraph, **The owner's answer (2026-11-24)**: the question as put ("A menu map says a track is a commentary
  or audio description (strong tie) and lazymkv's content check kept it as programme. What should lazymkv do with
  that track?"), the answer ("Drop it as confirmed"), and that the "Outranks" proposal's review-or-note reading is
  thereby replaced by a drop, with this plan's MR-2 to MR-16 as proposals. Section 7 "Made by the owner" gains that
  bullet. "Still unruled" gains: a track the menu and the content rule both call a commentary or an audio
  description (MR-12). Nothing else in the design changes.
- [ ] **Step 3: the main spec**, one dated sentence appended to each (no other edit to those paragraphs):
  - R6: `Since 2026-11-24 the disc's own menu confirms it too, where a menu map is given: a commentary or
    audio-description role with a strong tie on a track the content rule kept (menu maps design 5.4a).`
  - D12: `(Owner, 2026-11-21 and 2026-11-24.) A menu map's commentary or audio-description role with a strong tie
    outranks the content rule's keep and counts as confirmed: the track is dropped (menu maps design 5.4a;
    docs/menu-maps/format.md).`
  - D15: `Since 2026-11-24 a menu map's role with a strong tie also judges it, where the content rule says keep
    (D12).`
  - D23: `Since 2026-11-21 one case is ruled otherwise: a menu map's role with a strong tie outranks the content
    rule's keep (D12; menu maps design 5.4a). Every other case still follows this decision.`
  - 8.6 step 1, a new last bullet: `streams a menu map confirms as commentary or audio description (D12; menu maps
    design 5.4a): only where a map is given, its disc ID matches, the tie is strong and the content rule's verdict
    on the stream is keep; never the programme reference. The engine gives no map yet.`
  - The risk "Commentary on Blu-ray and UHD": `Where a menu map is given, a commentary the menu names with a strong
    tie is dropped as confirmed (D12).`
- [ ] **Step 4: `docs/phase1/decision-core.md`**, section "`lazymkv-dev decide`": a dated paragraph on `--menu`:
  what it reads, that the `decision` key can change, and where the rule is written.
- [ ] **Step 5: Check** `grep -rn "evidence only" docs crates --include=*.md --include=*.rs` and fix every line the
  increment made untrue. Run `CARGO_TEST` (a doc test or include may read a changed file).
- [ ] **Step 6: Commit** `docs: the menu-role rule, its ties and its rulings`.

### Task 7: acceptance on the 13 real maps (the controller, outside the repository)

Nothing here is committed but the counts in `format.md`. All files stay under `$SPIKE_DATA/out/menu-role/`.

- [ ] **Step 1:** Copy each of the 13 maps (`menu-map.json` only, checked against `maps.sha256`) and write its
  `menu-off.tsv` from the frozen decisions (`out/measure/off-parts.tsv`) with a script that calls the measurement's
  own Python to find, per audio choice, the decision that applied to it. Copy each recording's small files without
  `tmdb.json` (MR-16).
- [ ] **Step 2:** Build `lazymkv-dev` and run `decide` on each copy without and with `--menu`. Check: exit codes
  equal; every key but `decision` and `menu` byte-equal; `menu.roles.acts` true on 13 of 13.
- [ ] **Step 3:** Compare every `roles.audio` row (stream, role, tie) with the measurement's join rows for the same
  slug (`out/join-maps/rows.tsv`, audio rows with a label). Every difference is explained or is a defect to fix
  before the report.
- [ ] **Step 4:** List, by slug, every stream whose decision changed and every confirmation with another outcome;
  set the 14 disagreeing rows of the measurement beside it and give the reason for each that did not drop. The rule
  is not changed to make a row drop (design 5.4a).
- [ ] **Step 5:** Fill `format.md`'s dated acceptance paragraph (counts only) and the test counts; commit `docs:
  record the menu-role increment's tests and acceptance`. Run the final whole-branch review, then
  `superpowers:finishing-a-development-branch`, and report to the owner: what changed, the list of Step 4, and
  every ruling.
