# Menu map format v1

A menu map is one JSON file per disc that lists the choices of the disc's menu: what the menu calls each choice, and
which playlist and streams choosing it selects. It is made outside lazymkv, by the map writer (`menumap`, Python) and
its replay, and lazymkv reads it with `core::menu` (`read`, `validate`). This document is the format both sides keep
to. The design behind it is the menu maps design of 2026-11-17 (kept with the spike, not in this repository), sections
3, 5.1, 5.3 and 5.4. Every example here is made up.

## What a map is and is not

- A map says only: the menu calls this choice X, and choosing it selects stream N of playlist P. It never says what a
  track *is*.
- A choice that could not be confirmed is kept, with its level and a note. Nothing is left out silently and nothing is
  rounded up.
- A map holds disc titles and readings of the disc's own text, so maps live outside every repository. Repository
  files name a disc by its slug only.
- Making a map never needs the owner to type or photograph a menu.
- A map changes no decision of lazymkv. Its labels are evidence shown beside the decisions.

## The folder and the file

One folder per disc under a maps root outside every repository, named by the slug (`<root>/<slug>/`):

| Path | What it holds |
|---|---|
| `menu-map.json` | The map (`core::menu::FILE_NAME`) |
| `scripts/` | One key-press script per choice, and the start and play scripts |
| `img/` | A crop of each label as drawn |
| `replay.json` | The last replay's results (not read by lazymkv) |

The writer saves `menu-map.json` as UTF-8 (never escaped), indent 1, known keys in the order below, unknown keys after
them, and one trailing newline.

## Fields

`format` (string): the format name, `lazymkv menu map v1`.

`disc` (object):
- `slug` (string): the disc's slug, `^[a-z0-9][a-z0-9-]*$`.
- `disc_id_sha1` (string or null): the capture's disc ID (see Identity).
- `listing_sig` (string or null): the capture's listing signature (see Identity).
- `format` (string or null): the disc format, `uhd`, `bd` or `dvd` by the writer's convention (lazymkv does not check
  the word).

`made` (object, free-form for lazymkv): who made the map (`by`, for example `agent-explorer`), who checked it
(`checked_by`), the `date`, the driver's hash (`driver_sha256`), the `libbluray` and `java` versions, and the player
`profile` used (`region`, `country`, `menu_lang`, `audio_lang`, `pg_lang`). The profile is an input: another profile
may give another map.

`start` (object): how the menu was reached.
- `script` (relative path or null): the key presses that reach the menu.
- `passed` (list of strings): what had to be passed on the way, such as a language chooser and the answer given.
- `seconds_to_menu` (number or null): how long it took.

`play` (object): what plain Play does with no choice made.
- `label` (string): the label of Play as read, for example `PLAY`.
- `script` (relative path or null) and `effect` (effect or null).
- `check` (check).

`choices` (list of objects), one per choice:
- `id` (string): `c` and three digits, unique in the map.
- `section` (string): the section the choice sits in, as printed, headings joined with ` > ` (may be empty).
- `label` (string): the label as read.
- `label_image` (relative path or null): the crop of the label as drawn.
- `kind` (word), `surface` (word): see the word lists.
- `script` (relative path): the key presses of the choice.
- `applies` (word): see the word lists.
- `effect` (effect or null).
- `differs_from_play` (true, false or null): whether the effect differs from plain Play's (by `effects_equal`). The
  writer leaves it null when either effect is null; lazymkv checks it only when both effects are objects, so it does
  not check that convention.
- `check` (check).

An effect (object):
- `playlist` (string): the playlist as the disc names it, five digits (`00800`) by the writer's convention; lazymkv
  takes any string. A playlist that is not, ASCII case ignored, the stem of a scan title's source file
  (`<playlist>.mpls`) is reported `not in the scan`.
- `audio` (integer or null): the audio stream the player selects, counted from 1; null for no audio.
- `subtitle` (integer or null): the subtitle stream, counted from 1; null for none.
- `subtitle_shown` (true or false): whether that subtitle is shown. With subtitles off, `subtitle` is a leftover
  number and names no stream (M3-4).

Two effects are equal (`effects_equal`) when the playlist, the audio and `subtitle_shown` are the same, and, only when
both show their subtitle, the subtitle too.

A check (object):
- `label_reread` (string): the independent second reading of the label: `same`, or what the second reader read.
- `level` (word): see the ladder.
- `replays` (integer): how many deterministic replays ran.
- `replays_equal` (true, false or null): whether they agreed; null when none ran.
- `player` (true or false): the owner saw this on a real player (a separate mark, not a level).
- `note` (string): anything to say, parts joined with `; `; empty when nothing.

`not_mapped` (list of strings): what the map leaves out, by name (for example `SCENES`).

`quirks` (list of anything): anything a person would want to know. Later versions may add structured quirks.

A short made-up choice:

```json
{
 "id": "c002",
 "section": "SETUP > AUDIO",
 "label": "Language B 5.1",
 "label_image": "img/c002.png",
 "kind": "audio",
 "surface": "main",
 "script": "scripts/c002.txt",
 "applies": "at-play",
 "effect": {"playlist": "00800", "audio": 2, "subtitle": 3, "subtitle_shown": true},
 "differs_from_play": true,
 "check": {"label_reread": "same", "level": "replayed", "replays": 3, "replays_equal": true, "player": false,
           "note": ""}
}
```

## The word lists

- `kind`: `audio`, `subtitle`, `version`, `bonus`, `other`.
- `surface`: `main` (the main menu), `popup` (the pop-up menu), `other` (anywhere else, such as a bonus section).
- `level`, a ladder from low to high: `read` (one reader); `checked` (a second reader read the same label and its
  replay gave the same effect); `replayed` (checked, and the deterministic replay agreed at least 3 times,
  `REPLAYS_NEEDED`). By the writer's convention a choice whose two readings differ stays `read` however well it
  replays; lazymkv does not compare `label_reread` with the level. A map's own level is the lowest of play's and every
  choice's (`read` with no choices), as the writer's `map_level` counts it (design 5.1 names the choices only), and is
  only a summary.
- `applies`: `at-play` (takes effect when Play is chosen afterwards), `immediately` (at once).

## The writer's rules

`validate` gives every problem as one sentence each, in the writer's order, which is the order below (empty when the
map is valid). A value is quoted as Python's `repr` quotes it (`'text'`, `None`, `True`). "A relative path" is a
non-empty string that does not start with `/` or `\` or a drive letter (`C:`) and has no `..` part split on `/` and
`\`; the check is on the string, the same on every system. "A label" is a non-empty string with no control character
and no line or paragraph separator (U+2028, U+2029). "An integer" is never a float (`1.0`) and never true or false.
Each rule names the shared case that pins it (the cases file, `crates/core/src/menu/fixtures/menu-map-v1-cases.json`,
against the made-up map `menu-map-v1.json` beside it). Both validators, the writer's `menumap.validate` and lazymkv's
`core::menu::validate`, give each case's list exactly; lazymkv's `read` gives the same list on every case but
`format_wrong`, which it answers with the version sentence instead (M3-8).

Every key listed under Fields for `start`, `play` and a choice must be present: a missing one is `<place>: "<key>" is
missing` (`<place>` is `start`, `play`, or `choices[<i>] (<id>)`, `choices[<i>]` while the choice has no valid id), at
that key's rule below (`id_missing`, `play_check_missing`; the other keys have no shared case). An effect's four keys
and a check's six have their own sentences (below). `disc`'s keys other than `slug` may be left out and are read as
null; `made` is free-form.

- The map is a JSON object — `not_an_object`; nothing more is said of a map that is not.
- `format` is `lazymkv menu map v1` — `format_wrong`; a null or missing format prints `None` — `three_problems_listed`.
- Then the seven sections in this order: `disc`, `made`, `start`, `play`, `choices`, `not_mapped`, `quirks`. Each is
  present and has its type, an object, or a list for `choices`, `not_mapped` and `quirks` — `section_missing`,
  `section_wrong_type`, `quirks_wrong_type`. A section's own rules come right after its presence and type, before the
  next section's.
- `disc`: `slug` is present — `slug_missing`; and matches `^[a-z0-9][a-z0-9-]*$` — `slug_pattern`.
- `start`, in this order: `script` is null or a relative path — `start_script_absolute`, `start_script_drive`,
  `start_script_dotdot`; `passed` is a list of strings — `start_passed_not_strings`; `seconds_to_menu` is a finite
  number or null, never true or false — `start_seconds_bool`.
- `play`, in this order:
  - `label` is a label — `play_label_empty`, `play_label_control`;
  - `check` is present — `play_check_missing`; and passes the check rules (below);
  - `script` and `effect` are present; `script`, when not null, is a relative path (no shared case);
  - `script` is null only when `effect` is null too — `play_script_null_effect_not`; both are null together only at
    level `read` with a note — `play_both_null_not_read`; `effect` is null only at level `read` with a note —
    `play_effect_null_not_read`;
  - an `effect` that is not null passes the effect rules (below).
- Each choice of `choices` in turn, its rules in this order:
  - it is an object — `choice_not_object`;
  - `id` is present — `id_missing`; is `c` and three digits — `id_pattern`; and is not used twice (the later choice is
    named) — `id_twice`;
  - `section` is a string — `section_not_string`;
  - `label` is a label — `label_not_string`, `label_line_separator`;
  - `kind`, `surface` and `applies`, in that order, are words of their lists — `kind_surface_applies_unknown`;
  - `check` is present and passes the check rules (below) — `check_not_object` and the other check cases;
  - `label_image` is null only at level `read` — `label_image_null_above_read`; when not null it is a relative path
    — `label_image_dotdot`;
  - `script` is a relative path, never null — `choice_script_null`, `script_backslash`, `script_backslash_dotdot`,
    `script_empty`;
  - `effect` is null only at level `read` with a note — `choice_effect_null_not_read`; when not null it passes the
    effect rules (below);
  - `differs_from_play` is present; when the choice's effect and play's are both objects, it is true or false —
    `differs_not_bool`; and it is the opposite of `effects_equal` of the two (compared as Python compares the raw
    values, so `1`, `1.0` and `true` are equal and a missing key is null) — `differs_wrong`, and the last sentence of
    `effect_key_missing` and `effect_types`.
- `not_mapped` is a list of strings — `not_mapped_not_strings`.
- `quirks` is a list (its items are free-form) — `quirks_wrong_type`.
- Every problem is listed, not only the first — `three_problems_listed`.
- The writer only, last: a value that cannot be saved as JSON (not finite, a lone surrogate) is one more sentence,
  `map cannot be saved: ...`. No such value reaches lazymkv's reader (see Known reader differences).

The check rules, for play's check and each choice's, in this order: the check is an object — `check_not_object`; it
has all six keys (one sentence for each missing key) — `check_key_missing`; its `level` is one of the ladder's words —
`level_unknown`; level `replayed` needs `replays` an integer of at least 3 and `replays_equal` true —
`replayed_few_replays`, `replayed_replays_not_equal`; its `player` is true or false — `player_not_bool`; its `note` is
a string — `note_not_string`.

The effect rules, for play's effect and each choice's, in this order: the effect is an object — `effect_not_object`;
it has all four keys (one sentence for each missing key) — `effect_key_missing`; its `playlist` is a string, `audio`
and `subtitle` integers or null, and `subtitle_shown` true or false, in that order — `effect_types`,
`effect_audio_float`, `effect_audio_bool`.

## The reader rules

`read` takes the file's text. After the writer's rules pass, lazymkv's types need more of the fields the writer's
rules leave open; a map that breaks one is refused with one sentence, `the map passes the format's rules but not this
reader's: <the JSON library's error>`:
- `disc.disc_id_sha1`, `disc.listing_sig` and `disc.format` are strings or null.
- A check's `label_reread` is a string and its `replays` an integer (within the signed 64-bit range).
- A check's `replays_equal` and a choice's `differs_from_play` are true, false or null.
- `audio` and `subtitle` fit the signed 64-bit range.

A leading byte-order mark is dropped. Text that is not JSON gives one sentence, `not JSON: <the JSON library's
error>`. Keys the reader does not know are read and dropped, at every level, so a later writer may add keys.

## The version rule

A map whose `format` is a string other than `lazymkv menu map v1` is refused with exactly one sentence before any v1
rule speaks (M3-8):

    this lazymkv reads only 'lazymkv menu map v1'; the map says it is 'lazymkv menu map v2'

A `format` that is not a string (null, a number, missing) is no other version: the map gets every problem the
writer's rules give, the format's first.

## Identity

- `disc_id_sha1`: the SHA-1 of the disc's `AACS/Unit_Key_RO.inf`, or of `AACS/DUPLICATE/Unit_Key_RO.inf` when the
  first is missing, as 40 upper-case hex digits. It is the same value as a recording's `meta.json` `disc_id`, and the
  one value lazymkv checks a map's disc by (M3-6).
- `listing_sig`: the SHA-256 of the disc's listing, one `path|size` line per file, sorted. It is recorded and shown;
  lazymkv does not check it.

## Stream numbers

A map numbers streams as the player does, from 1, per playlist. lazymkv ties them to a scan title whose source file
is `<playlist>.mpls` (ASCII case ignored; every such title, when two share one source file, each on its own streams)
by MakeMKV's observed order, not a spec, and parses no playlist (M3-1):
- map audio n is the n-th audio stream of the scan title without flag 256 (`CORE_AUDIO`);
- map subtitle n is the n-th subtitle stream of the scan title without flag 2048 (`DERIVED`).

With subtitles off the subtitle number is a leftover and names no stream (M3-4). A playlist that is no title of the
scan is `not in the scan`, and its streams are not checked (M3-2).

## The check

`menu check` prints the identity, then `listing_sig: not checked (lazymkv does not recompute it)`, then one line per
entry (play first, then the choices in map order; one line per scan title of the entry's playlist) with ids,
playlists and numbers only, never a label. A scan that lists no title (a file that is not a scan, or a `scan.txt`
that is empty or holds no title) is refused before the check with `<scan path>: the scan lists no title`, exit 2, for
a bare scan file and a recording's `scan.txt` alike. The check's last line is

    CHECK ok entries=10 on_scan=8 not_in_scan=1 no_effect=1 warnings=0 failed=0; streams of not_in_scan entries were not checked

`ok` when `failed` is 0, else `FAILED`. The counts:
- `entries`: play and every choice.
- `on_scan`: the entries whose playlist is a title of the scan; their streams are checked.
- `not_in_scan`: the entries whose playlist is no title of the scan; their streams are not checked (M3-2), which the
  line's last words say.
- `no_effect`: the entries without an effect (`<id>: no effect recorded`).
- `warnings`, each one on each title's line: a subtitle that is off with a leftover number below 1 or above the
  subtitle count (M3-4); an audio choice with audio null, or a subtitle choice shown with subtitle null (M3-5). A null
  number on another kind of entry is no warning.
- `failed`, each one on each title's line: an audio number, or a shown subtitle number, below 1 or above the count of
  its kind (`OUT OF RANGE (<k> audio streams)`, `OUT OF RANGE (<k> subtitle streams)`); plus 1 when the map's
  `disc_id_sha1` differs from the disc's ID. An identity that is not checked is no failure (M3-6).

## Exit codes

`lazymkv-dev menu show <map>` and `lazymkv-dev menu check <map> --scan <slug|recording dir|scan file>
[--evidence DIR]` (`<map>` is a map folder or its `menu-map.json`; `--scan` is a recording folder, a slug under the
evidence clone, or a bare scan file, which carries no disc ID):
- `menu show`: 0 when the map is printed, 2 for an argument or map problem.
- `menu check`: 0 when nothing failed (`failed=0`), 1 otherwise, 2 for an argument, map or scan problem (a scan that
  lists no title included), 3 for a slug without the evidence clone.

A refusal prints `lazymkv-dev menu: error: <line>` on stderr, one line for each problem of the map, as
`<path>: <problem>`. A scan that cannot be typed or lists no title, or a `meta.json` that cannot be read, is one such
line too; a scan file that cannot be read prints the system's error text, which names its path. A usage error (a
missing, extra or unknown argument) prints a usage line first, then one line `lazymkv-dev menu: error: <message>`,
`lazymkv-dev menu show: error: <message>` or `lazymkv-dev menu check: error: <message>`, as argparse words it.

## How decide shows a map

`lazymkv-dev decide ... --menu MAP` (`MAP` is a map folder or its `menu-map.json`) adds one last key, `menu`, to
decide's document. It is evidence only: with a map that can be read, every other key and the exit code are those of
the run without `--menu` (the other keys byte for byte), also when the recording's `meta.json` cannot be read, and no
verdict, role, plan or decision reads the map. Without `--menu` the key is absent; the engine never sets it, so its
decide.json files are unchanged. The key is one of:

- the block of the rip label's title (`core::menu::title_block`), its keys in this order:
  - `map` (the map's slug) and `map_level`;
  - `identity`: the text after `identity: ` in `menu check`'s first line, `disc_id matches (<id>)` or `not checked:
    <reason>`;
  - `playlist`: the title's source file without `.mpls`, or null for a title without a source file;
  - `starts`: play and the version, bonus and other choices on that playlist (M3-3), as labels;
  - `audio`: a list of `{"stream", "labels"}`, one for each scan stream that an audio choice's own audio number names
    (M3-1), sorted by `stream` (the scan stream number), the labels in map order;
  - `subtitles`: the same for the shown subtitle choices and their subtitle numbers;
  - `subtitles_off`: the subtitle choices that turn subtitles off (M3-4), as labels;
  - `other_playlists`: every other playlist the map's effects name, as `{"playlist", "entries", "in_scan"}` (how many
    entries name it, and whether it is a title of the scan, M3-2), in first-seen order, play first;
  - `out_of_range`: the ids of the own-kind choices whose number names no stream (M3-5);
  - `no_effect`: the ids of the entries without an effect.

  A label is `{"id", "label", "level", "differs_from_play"}`, the same four keys in `starts`, in `subtitles_off` and in
  each row's `labels`. Play's has the id `play`, play's label alone, its level and `differs_from_play` null; a
  choice's has its id, its shown label, its level and its `differs_from_play` as the map says.
- `{"skipped": "<path>: <problem>"}` when the recording's `meta.json` cannot be read, the line `menu check` would
  refuse with;
- `{"skipped": "the map is for another disc: disc_id differs: the map's is <a>, the disc's is <b>"}` when the map's
  `disc_id_sha1` differs from the recording's `meta.json` `disc_id` (M3-6);
- `{"skipped": "<sentence>"}` when the scan cannot be typed, or `the scan lists no title <n>`.

A block's `label` is `Choice::shown_label`, the whole section joined to the label (`SETUP > AUDIO: Language B 5.1`),
not the measurement's text (innermost heading only, white space collapsed, `|` written `/`; design 5.4a).

A map that cannot be read or does not pass the format's rules exits 2 with `lazymkv-dev decide: error: <line>`, one
line for each problem, and no document. `lazymkvctl` relays a relative `--menu` made absolute against its own folder,
as it does decide's other path flags (V10).

## Shown labels

lazymkv shows a choice as `SECTION: label` (the whole section, or the label alone when the section is empty;
`Choice::shown_label`), for example `SETUP > AUDIO: Language B 5.1`; this is not the measurement's text, which keeps
only the innermost heading, collapses white space and writes `|` as `/`.

## Known reader differences

Both validators (`menumap.validate` and `core::menu::validate`) give the same list on every case of the shared file
(M3-7); lazymkv's `read` gives it too, but for `format_wrong`, which it answers with the version sentence (M3-8).
Outside the shared file the two readers differ here, as observed with lazymkv's reader:
- A leading byte-order mark: the Python loader refuses the file; the Rust reader drops the mark and reads on.
- A lone-surrogate escape (`"\ud800"`), `NaN` or `Infinity`: Python reads them and its validate reports them (`map
  cannot be saved`); the Rust reader gives `not JSON: ...`. A number too large for a float (`1e400`) is the same
  case: Python reads an infinity, the Rust reader gives `not JSON: number out of range ...`.
- `-0`: Python reads the integer 0; the Rust JSON library reads the float -0.0. Where validate checks an integer it
  then reports a type error (`effect audio is not a number or null` for an `audio` of `-0`); in a check's `replays`
  it gives the replayed sentence at level `replayed`, and below that level the reader refuses it with its reader
  sentence.
- Integers. Within the signed 64-bit range both read them as integers, but for `-0` (above). Above it but within the
  unsigned 64-bit range (`9223372036854775808` to `18446744073709551615`) the writer's rules pass in both, and the
  Rust reader then refuses with its reader sentence (`invalid value: integer ..., expected i64`) where the field is
  typed, such as `audio`. Beyond the unsigned 64-bit range, or below the signed one, the Rust JSON library reads the
  number as a float, so validate reports a type error (`effect audio is not a number or null`); Python accepts it as
  an integer.

## Rulings

Increment 3's rulings (the plan of 2026-11-17, amended 2026-11-21), verbatim:

- M3-1: map audio n is the n-th audio stream of the scan title without `CORE_AUDIO`, subtitle n the n-th subtitle
  stream without `DERIVED`: MakeMKV's observed order (244 of 244 effects on a scan title of the 13 maps), not a spec;
  no playlist parser.
- M3-2 (departure from spec 5.4's "every stream it names exists in the scan"): an effect on a playlist that is no
  title of the scan is reported `not in the scan`, its streams are not checked, and it keeps exit 0; the CHECK line
  says so. A strict check would fail 7 of the 13 real maps. Owner's question c in Task 0.
- M3-3 (a gap in spec 5.3, which names no stream for these): a version, bonus or other choice labels no stream in
  decide's block, including the 24 real other-kind choices that set audio and subtitles on the ripped playlist; they
  appear under `starts` or `other_playlists` only. Owner's question c in Task 0.
- M3-4: subtitles off leave a leftover number: never a label; above the table it is a warning, never a failure.
- M3-5: an audio choice with audio null, or a subtitle choice shown with subtitle null, names no stream of its kind: a
  warning in check and its id under `out_of_range` in the block, never a failure.
- M3-6: identity is `disc_id_sha1` against a recording's `meta.json` `disc_id` only; `listing_sig` is shown and not
  checked.
- M3-7: both readers give the same list on every case of the shared file. Known differences outside it, listed in the
  format document: a leading BOM (Python's loader refuses, the Rust reader drops it); a lone-surrogate escape, `NaN` or
  `Infinity` (Python reads them and validate reports them; Rust's parser gives `not JSON`); an integer outside i64
  (Python accepts it; Rust reports a type error).
- M3-8: a map of another format version is refused with one sentence before any v1 rule speaks.

The owner's answers of 2026-11-21: question c is `keep`, so M3-2 and M3-3 stand as written. The builds run on the Mac
(the plan's questions a, b and d were about the mini PC and fell away).

Added on 2026-11-21:
- M3-6 is also a departure from spec 5.4's "belongs to the scanned disc": when either ID is missing the first line
  says `identity: not checked: <reason>` and the check can still pass.
- M3-4 is a reading within the spec: with subtitles off the effect shows no stream, so the map names none.
- `Choice::shown_label` is the whole section joined to the label (`SETUP > AUDIO: Language B 5.1`): what a person is
  shown. It is not the measurement's text (innermost heading only, white space collapsed, `|` written `/`; design
  5.4a).

## Tests and acceptance

Recorded on 2026-11-21 for the branch that adds the reader, `menu show`, `menu check` and `decide --menu`.

- Tests: `cargo test --workspace --exclude lazymkv-ocr --locked --offline` (CI's macOS command, run here with
  `--offline`), run on this Mac with toolchain 1.100.0: 3381 passed, 0 failed, 5 ignored, in 57 result lines, with no
  warning. 48 tests print `skipped:` on this Mac because the evidence clone is absent; CI's counts differ per system.
  `main` before the branch gave 3327 passed with the same command, so the branch adds 54 tests: 26 in `core::menu`, 19
  in the `menu` command, 8 in decide and 1 in lazymkvctl. Nothing has run on Windows or Linux yet; that is CI's part
  once the branch is pushed.
- Acceptance on the 13 real maps, each against its recording's scan and `meta.json` (counts only): `menu check` exits 0
  on 13 of 13 and says `identity: disc_id matches` on 13 of 13; `menu show` exits 0 on 13 of 13. Summed over the maps:
  entries 340, on_scan 244, not_in_scan 96 (on 7 maps), no_effect 0, warnings 0, failed 0.
- The two shared fixtures are byte-identical to the writer's copies.
