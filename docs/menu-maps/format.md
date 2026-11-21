# Menu map format v1

A menu map is one JSON file per disc that lists the choices of the disc's menu: what the menu calls each choice, and
which playlist and streams choosing it selects. It is made outside lazymkv, by the map writer (`menumap`, Python) and
its replay, and lazymkv reads it with `core::menu` (`read`, `validate`). This document is the format both sides keep
to. The design behind it is the menu maps design (`docs/superpowers/specs/2026-11-17-menu-maps-design.md`), sections
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
- `format` (string or null): the disc format, `uhd`, `bd` or `dvd`.

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
- `kind` (word), `surface` (word), `applies` (word): see the word lists.
- `script` (relative path): the key presses of the choice.
- `effect` (effect or null).
- `differs_from_play` (true, false or null): whether the effect differs from plain Play's (by `effects_equal`).
- `check` (check).

An effect (object):
- `playlist` (string): the playlist as the disc names it, five digits (`00800`).
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
  `REPLAYS_NEEDED`). A choice whose two readings differ stays `read` however well it replays. A map's own level is the
  lowest of play's and every choice's (`read` with no choices) and is only a summary.
- `applies`: `at-play` (takes effect when Play is chosen afterwards), `immediately` (at once).

## The writer's rules

`validate` gives every problem, in this order, as one sentence each (empty when the map is valid). A value is quoted
as Python's `repr` quotes it (`'text'`, `None`, `True`). "A relative path" is a non-empty string that does not start
with `/` or `\` or a drive letter (`C:`) and has no `..` part split on `/` and `\`; the check is on the string, the
same on every system. "A label" is a non-empty string with no control character and no line or paragraph separator
(U+2028, U+2029). "An integer" is never a float (`1.0`) and never true or false. Each rule names the shared case that
pins it (the cases file, `crates/core/src/menu/fixtures/menu-map-v1-cases.json`, against the made-up map
`menu-map-v1.json` beside it; both readers must give each case's list exactly).

- The map is a JSON object — `not_an_object`.
- `format` is `lazymkv menu map v1` — `format_wrong`; a null or missing format prints `None` — `three_problems_listed`.
- `disc`, `made`, `start`, `play`, `choices`, `not_mapped` and `quirks` are present — `section_missing`.
- Each has its type: an object, or a list for `choices`, `not_mapped` and `quirks` — `section_wrong_type`,
  `quirks_wrong_type`.
- `disc.slug` is present — `slug_missing`.
- `disc.slug` matches `^[a-z0-9][a-z0-9-]*$` — `slug_pattern`.
- `start.script` is null or a relative path — `start_script_absolute`, `start_script_drive`, `start_script_dotdot`.
- `start.passed` is a list of strings — `start_passed_not_strings`.
- `start.seconds_to_menu` is a finite number or null, never true or false — `start_seconds_bool`.
- `play.label` is a label — `play_label_empty`, `play_label_control`.
- `play.check` is present — `play_check_missing`.
- `play.script` is null only when `play.effect` is null too — `play_script_null_effect_not`.
- `play.script` and `play.effect` are null together only at level `read` with a note — `play_both_null_not_read`.
- `play.effect` is null only at level `read` with a note — `play_effect_null_not_read`.
- An effect is an object — `effect_not_object`.
- An effect has all four keys — `effect_key_missing`.
- An effect's `playlist` is a string, `audio` and `subtitle` integers or null, `subtitle_shown` true or false —
  `effect_types`, `effect_audio_float`, `effect_audio_bool`.
- A check is an object — `check_not_object`.
- A check has all six keys — `check_key_missing`.
- A check's `level` is one of the ladder's words — `level_unknown`.
- Level `replayed` needs `replays` an integer of at least 3 and `replays_equal` true — `replayed_few_replays`,
  `replayed_replays_not_equal`.
- A check's `player` is true or false — `player_not_bool`.
- A check's `note` is a string — `note_not_string`.
- A choice is an object — `choice_not_object`.
- A choice's `id` is present — `id_missing`.
- A choice's `id` is `c` and three digits — `id_pattern`.
- No `id` is used twice (the later choice is named) — `id_twice`.
- A choice's `section` is a string — `section_not_string`.
- A choice's `label` is a label — `label_not_string`, `label_line_separator`.
- A choice's `kind`, `surface` and `applies` are words of their lists — `kind_surface_applies_unknown`.
- A choice's `label_image` is null only at level `read` — `label_image_null_above_read`.
- A choice's `label_image`, when not null, is a relative path — `label_image_dotdot`.
- A choice's `script` is a relative path, never null — `choice_script_null`, `script_backslash`,
  `script_backslash_dotdot`, `script_empty`.
- A choice's `effect` is null only at level `read` with a note — `choice_effect_null_not_read`.
- When the choice's effect and play's are both objects, `differs_from_play` is true or false — `differs_not_bool`.
- ... and it is the opposite of `effects_equal` of the two (compared as Python compares the raw values, so `1`,
  `1.0` and `true` are equal and a missing key is null) — `differs_wrong`, and the second sentence of
  `effect_key_missing` and `effect_types`.
- `not_mapped` is a list of strings — `not_mapped_not_strings`.
- Every problem is listed, not only the first — `three_problems_listed`.
- The writer only: a value that cannot be saved as JSON (not finite, a lone surrogate) is one more sentence, `map
  cannot be saved: ...`. No such value reaches lazymkv's reader (see Known reader differences).

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
playlists and numbers only, never a label. Its last line is

    CHECK ok entries=10 on_scan=8 not_in_scan=1 no_effect=1 warnings=0 failed=0; streams of not_in_scan entries were not checked

`ok` when `failed` is 0, else `FAILED`. The counts:
- `entries`: play and every choice.
- `on_scan`: the entries whose playlist is a title of the scan; their streams are checked.
- `not_in_scan`: the entries whose playlist is no title of the scan; their streams are not checked (M3-2), which the
  line's last words say.
- `no_effect`: the entries without an effect (`<id>: no effect recorded`).
- `warnings`, each one on each title's line: a subtitle that is off with a leftover number below 1 or above the subtitle count (M3-4); an
  audio choice with audio null, or a subtitle choice shown with subtitle null (M3-5). A null number on another kind
  of entry is no warning.
- `failed`, each one on each title's line: an audio number, or a shown subtitle number, below 1 or above the count of its kind
  (`OUT OF RANGE (<k> audio streams)`, `OUT OF RANGE (<k> subtitle streams)`); plus 1 when the map's
  `disc_id_sha1` differs from the disc's ID. An identity that is not checked is no failure (M3-6).

## Shown labels

lazymkv shows a choice as `SECTION: label` (the whole section, or the label alone when the section is empty;
`Choice::shown_label`), for example `SETUP > AUDIO: Language B 5.1`; this is not the measurement's text, which keeps
only the innermost heading, collapses white space and writes `|` as `/`.

## Known reader differences

Both readers give the same list on every case of the shared file (M3-7). Outside it they differ here, as observed
with lazymkv's reader:
- A leading byte-order mark: the Python loader refuses the file; the Rust reader drops the mark and reads on.
- A lone-surrogate escape (`"\ud800"`), `NaN` or `Infinity`: Python reads them and its validate reports them (`map
  cannot be saved`); the Rust reader gives `not JSON: ...`. A number too large for a float (`1e400`) is the same
  case: Python reads an infinity, the Rust reader gives `not JSON: number out of range ...`.
- Integers. Within the signed 64-bit range both read them. Above it but within the unsigned 64-bit range
  (`9223372036854775808` to `18446744073709551615`) the writer's rules pass in both, and the Rust reader then refuses
  with its reader sentence (`invalid value: integer ..., expected i64`) where the field is typed, such as `audio`.
  Beyond the unsigned 64-bit range, or below the signed one, the Rust JSON library reads the number as a float, so
  validate reports a type error (`effect audio is not a number or null`); Python accepts it as an integer.

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
