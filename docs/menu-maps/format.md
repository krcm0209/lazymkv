# Menu map format v1

A menu map is one JSON file per disc that lists the choices of the disc's menu: what the menu calls each choice, and
which playlist and streams choosing it selects. It is made outside lazymkv, by the map writer (`menumap`, Python) and
its replay, and lazymkv reads it with `core::menu` (`read`, `validate`). This document is the format both sides keep
to. The design behind it is the menu maps design of 2026-11-17, `docs/superpowers/specs/2026-11-17-menu-maps-design.md`,
sections 3, 5.1, 5.3, 5.4 and 5.4a; since 2026-11-24 also the engine menu maps design,
`docs/superpowers/specs/2026-11-24-engine-menu-maps-design.md`, parts 8, 9, 12 and 13, as its pull request 1 builds
them. Every example here is made up.

## What a map is and is not

- A map says only: the menu calls this choice X, and choosing it selects stream N of playlist P. It never says what a
  track *is*.
- A choice that could not be confirmed is kept, with its level and a note. Nothing is left out silently and nothing is
  rounded up.
- A map holds disc titles and readings of the disc's own text, so maps live outside every repository. Repository
  files name a disc by its slug only.
- Making a map never needs the owner to type or photograph a menu.
- A map changes the audio decision of lazymkv, and only where `lazymkv-dev decide --menu` gives it (the engine gives
  no map): an audio track the menu calls a commentary or an audio description, with a strong tie, on a map whose
  identity matches, is dropped under the guards of The rule, whatever the content rule says of it and also without a
  verdict (design 5.4a). Amended on 2026-11-24: a map can also hold the job with `audio_reference_unclear`, when it
  calls the programme reference a commentary or an audio description or when its drop would move the reference
  (EO-5), and it adds notes to the decision (The rule). Every other label is evidence shown beside the decisions.

## The folder and the file

One folder per disc under a maps root outside every repository, named by the slug (`<root>/<slug>/`):

| Path | What it holds |
|---|---|
| `menu-map.json` | The map (`core::menu::FILE_NAME`) |
| `menu-off.tsv` | The off-decisions, when there are any (`core::menu::OFF_FILE_NAME`; The off-decisions file) |
| `scripts/` | One key-press script per choice, and the start and play scripts |
| `img/` | A crop of each label as drawn |
| `replay.json` | The last replay's results (not read by lazymkv) |

The writer saves `menu-map.json` as UTF-8 (never escaped), indent 1, known keys in the order below, unknown keys after
them, and one trailing newline.

Since 2026-11-24 a map can be published (`menu publish`, Exit codes; engine menu maps design part 8): its
`menu-map.json` and `menu-off.tsv`, and nothing else of its folder, are copied byte for byte into a maps folder, one
folder per disc named by the disc's ID, not by the slug (`<maps root>/<ID>/`). `<ID>` is the map's
`disc.disc_id_sha1` with surrounding white space left off, in upper case, and only when it is exactly 40 ASCII hex
digits (`core::menu::disc_folder`); it is the only part of a map a path is built from. A map file over 4 MiB
(`core::menu::MAP_MAX_BYTES`, 4194304 bytes) and an off file over 1 MiB (`core::menu::OFF_MAX_BYTES`, 1048576 bytes)
are not published.

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
by MakeMKV's observed order, not a spec (M3-1). The order tie is still M3-1's; for the menu role the playlist checks it
(Playlists, Ties, MR-9):
- map audio n is the n-th audio stream of the scan title without flag 256 (`CORE_AUDIO`);
- map subtitle n is the n-th subtitle stream of the scan title without flag 2048 (`DERIVED`).

With subtitles off the subtitle number is a leftover and names no stream (M3-4). A playlist that is no title of the
scan is `not in the scan`, and its streams are not checked (M3-2).

## The check

`menu check` prints the identity, then `listing_sig: not checked (lazymkv does not recompute it)`, then one line per
entry (play first, then the choices in map order; one line per scan title of the entry's playlist) with ids,
playlists and numbers only, never a label. A playlist other than five ASCII digits, and a disc ID other than ASCII
hex digits (the map's or the recording's), is printed quoted and escaped as Python's `repr` writes it, so that no text
of a map or a `meta.json` can break a line or forge one. A scan that lists no title (a file that is not a scan, or a
`scan.txt` that is empty or holds no title) is refused before the check with `<scan path>: the scan lists no title`,
exit 2, for a bare scan file and a recording's `scan.txt` alike. The check's last line is

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
- `menu show`: 0 when the map is printed, 2 for an argument or map problem. It prints the map's text as it is, except
  that free text holding a control character or a line or paragraph separator (U+2028, U+2029), what the writer keeps
  out of a label but not out of a section, a note, a script, a `made` value or a `not_mapped` item, is quoted and
  escaped as Python's `repr` writes it, and the playlists, `disc_id_sha1` and `listing_sig` print as `menu check`
  prints playlists and IDs.
- `menu check`: 0 when nothing failed (`failed=0`), 1 otherwise, 2 for an argument, map or scan problem (a scan that
  lists no title included), 3 for a slug without the evidence clone.

Since 2026-11-24 (engine menu maps design parts 8 and 9), also `lazymkv-dev menu off <map>` and `lazymkv-dev menu
publish <map> --to ROOT` (`<map>` as for `menu show`; the off-decisions beside it are read as `decide --menu` reads
them, The off-decisions file):
- `menu off`: 0 when the lines are printed, 2 for an argument problem, or a map or off file that cannot be read or
  does not pass the rules. It prints one line for each audio choice, in map order, that a decision of the off file
  names with another text, whatever its role words (the lines `menu publish` refuses as stale; since 2026-11-24,
  Ruling FR-2), or whose text (`role_text`, Roles) holds a role word and that the net catches or a decision of the off
  file names: `<state><TAB><id><TAB><text>`, `<text>` the choice's text and `<state>` one of `off` or `role` (a
  decision fits the text), `stale` (a decision names the id with another text) or `undecided`. A line saved in
  `menu-off.tsv` with its first field `off` or `role` decides that choice (a `stale` one in place of the old line).
  Made up: `undecided<TAB>c012<TAB>COMMENTARY: Off`.
- `menu publish`: 0 when the map is published, 2 for an argument problem, when a check fails, or when a write fails
  after the checks. Nothing is written when a check fails; a write that fails after the checks may leave
  `<ROOT>/<ID>/` or the new map in place (amended on 2026-11-24). The checks, every problem one refusal line, in this
  order: the map's size, then its read and format problems as `menu show` prints them; the off file's size, then its
  read and parse problems as `decide --menu` prints them; `<map path>: the map has no disc ID, so no job could find
  it` when `disc_folder` gives none; `<off path>: <id> is stale: the map's text is <text>, the line's is <text>` for
  each stale line, ascending id, the two texts quoted as Python's `repr` writes them (EO-12, EM-19); `<ROOT>: not a
  folder` (ROOT as given, which must exist: it is never made); `<ROOT>/<ID>: not a folder` when that name is a file. A
  file over its limit is refused as `<path>: <n> bytes is over the limit of <limit>` (its size read from the file
  system, the file not read). An unused line is no refusal and is published as it is (EO-13, EM-22). When every check
  passes it makes `<ROOT>/<ID>/` when it is missing, writes the map's bytes unchanged to `<ROOT>/<ID>/menu-map.json`
  and, when the map has an off file, its bytes to `<ROOT>/<ID>/menu-off.tsv`, each to a temporary file in that folder
  and then put in place (`lazymkv_platform::write_atomic`), removes a `menu-off.tsv` there when the map has none, and
  prints one line, counts and the ID only, never a label:

      PUBLISHED 0123456789ABCDEF0123456789ABCDEF01234567 choices=12 decisions=2 undecided=0

  `choices` is the map's choices, `decisions` the lines `menu off` would print `off` or `role`, `undecided` the lines
  it would print `undecided`. It touches no other disc's folder and deletes no folder. A write that fails after the
  checks is one refusal line, `<path>: <the system's error>`, exit 2.

A refusal prints `lazymkv-dev menu: error: <line>` on stderr, one line for each problem of the map, as
`<path>: <problem>`. A scan that cannot be typed or lists no title, or a `meta.json` that cannot be read, is one such
line too (a scan's refusal quoted and escaped as Python's `repr` writes it when the scan text it names would break the
line); a scan file that cannot be read prints the system's error text, which names its path. A usage error (a
missing, extra or unknown argument) prints a usage line first, then one line `lazymkv-dev menu: error: <message>`,
`lazymkv-dev menu show: error: <message>`, `lazymkv-dev menu check: error: <message>`, `lazymkv-dev menu off: error:
<message>` or `lazymkv-dev menu publish: error: <message>`, as argparse words it. The usage lines are `menu [-h]
{show,check,off,publish} ...`, `menu show [-h] map`, `menu check [-h] map --scan SCAN [--evidence DIR]`, `menu off
[-h] map` and `menu publish [-h] map --to ROOT`, each after `usage: lazymkv-dev `.

## How decide shows a map

`lazymkv-dev decide ... --menu MAP` (`MAP` is a map folder or its `menu-map.json`) adds one last key, `menu`, to
decide's document. Since the menu-role increment (2026-11-24) it also reads the off-decisions beside the map (The
off-decisions file) and the recording's playlists (Playlists), and the `decision` key can change (The rule). With a
map and an off-decisions file that can be read, every key but `decision` and `menu`, and the exit code, are those of
the run without `--menu` (byte for byte), also when the recording's `meta.json` cannot be read; no verdict and no plan
reads the map. The `decision` key differs only where the block's `roles` act and confirm a stream; otherwise it
too is byte for byte the one without `--menu`. Without `--menu` the key is absent; the engine never sets it, so its
decide.json files are unchanged. The key is one of:

- the block of the rip label's title (`core::menu::title_block`), its keys in this order:
  - `map` (the map's slug) and `map_level`;
  - `identity`: the text after `identity: ` in `menu check`'s first line, `disc_id matches (<id>)` or `not checked:
    <reason>` (an ID quoted as `menu check` quotes it);
  - `playlist`: the title's source file without `.mpls`, or null for a title without a source file;
  - `starts`: play and the version, bonus and other choices on that playlist (M3-3), as labels;
  - `audio`: a list of `{"stream", "labels"}`, one for each scan stream that an audio choice's own audio number names
    (M3-1), sorted by `stream` (the scan stream number), the labels in map order;
  - `subtitles`: the same for the shown subtitle choices and their subtitle numbers;
  - `subtitles_off`: the subtitle choices that turn subtitles off (M3-4), as labels;
  - `other_playlists`: every other playlist the map's effects name, as `{"playlist", "entries", "in_scan"}` (how many
    entries name it, and whether it is a title of the scan, M3-2), in first-seen order, play first; playlists that
    differ in ASCII case alone are one row, spelt as first seen, as they name one source file;
  - `out_of_range`: the ids of the own-kind choices whose number names no stream (M3-5);
  - `no_effect`: the ids of the entries without an effect;
  - `roles` (since 2026-11-24): the title's audio roles (`menu::audio_roles`, `RolesReport::block`; Roles, Ties), its
    keys in this order:
    - `acts`: true when the roles may reach the decision: the title's own playlist was read and its play items carry
      one stream table (MR-8), and `identity` is `disc_id matches` (MR-5);
    - `why`: empty when they act; else why no stream of the title is tied (Playlists), or `the map's identity is
      <identity>` (`the map's identity is not checked: the recording has no meta.json`);
    - `audio`: one row for each scan audio stream that at least one audio choice labels, ascending, as `{"stream",
      "role", "tie", "choices", "play", "why"}`: the scan stream, its role (`commentary`, `audio_description`,
      `programme`, or null without exactly one role), its tie (Ties), the ids of the choices that label it in map
      order, whether it is the stream plain Play itself selects (Ties, "Play's own stream"; such a row never
      confirms, MR-17), and why it confirms nothing (Roles; empty when the role is set, the tie strong and the row is
      not a commentary or audio description on Play's own stream);
    - `unplaced`: the audio choices that label no stream of the title, in map order, as `{"id", "why"}`;
    - `off_unused`: the ids of the off-decisions that are not stale and name no audio choice of the map, or one whose
      text holds no role word, ascending (EM-22);
    - `off_stale` (since 2026-11-24, EP-1): the ids of the stale off-decisions, whose line's text differs from the
      text of the audio choice they name, ascending (The off-decisions file); it never shares an id with `off_unused`;
    - `play` (since 2026-11-24, EP-1): the scan stream plain Play itself selects (Ties, "Play's own stream"; MR-17), or
      null when it is not known and when no stream of the title is tied (Playlists). The decision reads it only when
      the roles act (The rule, EO-3).

    The two new keys are always written (`[]`, `null`), and a document without them is read as if they were `[]` and
    `null`.

    Only a block has `roles`; a `skipped` key has none and confirms nothing. `other_playlists` still counts a twin
    playlist's choices (M3-2 stands); `roles` holds them translated to the title's streams (MR-15).

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
line for each problem, and no document; so does a `menu-off.tsv` beside it that cannot be read or parsed (The
off-decisions file). `lazymkvctl` relays a relative `--menu` made absolute against its own folder, as it does decide's
other path flags (V10).

A made-up `roles` key, for a map whose c001 selects Play's own audio and whose c002 (`AUDIO: Commentary by Person A`)
selects audio 2, both at level `replayed`, and whose c004 selects a playlist that is not a twin:

```json
"roles": {
 "acts": true,
 "why": "",
 "audio": [
  {"stream": 1, "role": null, "tie": "default", "choices": ["c001"], "play": true, "why": "no role word"},
  {"stream": 3, "role": "commentary", "tie": "replay", "choices": ["c002"], "play": false, "why": ""}
 ],
 "unplaced": [{"id": "c004", "why": "other playlist 00801"}],
 "off_unused": [],
 "off_stale": [],
 "play": 1
}
```

The `decision` key gains two keys, each written only when it is not empty, so a decision with no confirmation prints as
before:
- `decision.facts.menu_streams`: the scan streams the block confirms as a commentary or an audio description (the
  ones the decision is given; `FactsSummary.menu_streams`), ascending;
- `decision.decision.audio.menu`: every confirmation the decision was given, ascending stream, with what became of it
  (`AudioDecision.menu`, MR-11), as `{"stream", "role", "choices", "outcome"}` (`MenuApplied`); it carries no content
  verdict, which no longer decides the outcome (MA-3; The rule).

A stream the menu drops is listed under `decision.decision.audio.dropped` as
`[<stream>, {"why": "menu", "role": "<role>"}]` (`AudioDrop::Menu`), in scan order with the other drops. Made up, for
the block above, where stream 3 is kept, has no Keep override and is not the reference, and the reference stays when
it is dropped, whatever the content rule's verdict on it:

```json
"dropped": [[3, {"why": "menu", "role": "commentary"}]],
"menu": [{"stream": 3, "role": "commentary", "choices": ["c002"], "outcome": "dropped"}]
```

Since 2026-11-24 the drop also raises a note (EO-2; The rule), which `decision.decision.notes` lists after the
audio decision's own notes and before the subtitles':

```json
{"note": "menu_dropped", "stream": 3,
 "detail": "the disc's menu calls stream 3 a commentary (\"AUDIO: Commentary by Person A\"); dropped on the menu's word"}
```

## Shown labels

lazymkv shows a choice as `SECTION: label` (the whole section, or the label alone when the section is empty;
`Choice::shown_label`), for example `SETUP > AUDIO: Language B 5.1`; this is not the measurement's text, which keeps
only the innermost heading, collapses white space and writes `|` as `/`.

## Roles

What a menu calls an audio stream (`core::menu` `role.rs`; design 5.4a "Menu role", MR-7, MR-10) is read as the
measurement read it, on audio choices only. The shared cases (`crates/core/src/menu/fixtures/menu-role-cases.json`,
made-up texts with the answers of the measurement's Python) pin the text, the role words and the net.

- **The text** of a choice (`role_text`) is the measurement's, not the shown label: the innermost heading of the
  section (after the last ` > `, white space trimmed), `: `, the label; the label alone when that heading is empty;
  every `|` written `/`; every run of white space one space; trimmed. `SETUP > AUDIO` and `Commentary by Person A`
  give `AUDIO: Commentary by Person A`.
- **The roles and their words**, matched on the lower-cased text (`audio_roles_in`); "at word edges" means that
  neither neighbour is a word character (alphanumeric or `_`):
  - `commentary`: `commentar` anywhere;
  - `audio_description` (the measurement's `ad`, MR-10): `audio descri`, `audio-descri`, `descriptive`, `visually
    impaired` or `visually-impaired` anywhere, or `described`, `dvs` or `ad` at word edges;
  - `programme`: `main`, `programm` or `programme` at word edges. `Program` gives no role, as the measurement's Python
    gives it none (a shared case), although design 5.4a names the word program.

  A text may hold several roles (`Main Feature with Commentary` holds `commentary` and `programme`).
- **The net** (`needs_decision`, MR-7): a text that holds a role word, and holds one of `off`, `no`, `none`, `not`,
  `without`, `disable`, `disabled`, `stop`, `hide` or `hidden` at word edges, in its lower-cased form or in its NFKC
  form lower-cased with white space collapsed, may name a role only to switch it off (`COMMENTARY: Off`). Its words
  are read only through an off-decision (The off-decisions file).
- **A stream's role** (`stream_role`) comes from the audio choices that label it, in map order. A text with no role
  word is skipped, and so is a text decided `off`. A text decided `role` is read. A text the net catches is undecided
  without a decision; any other text is read. Since 2026-11-24 a text is decided only by a decision that fits it
  (`OffDecisions::fits`): a stale line decides nothing, so a text the net catches is then undecided too. Any undecided
  text leaves the stream without a role, whatever the other texts say (MR-6). Else the one distinct role read is the
  stream's role; none, or several different ones, give no role.

A stream's `why` in the `roles` key says why the row confirms nothing. Its forms, in the order the code tests them:
the conflict texts of a `tie-conflict` (Ties); then why the stream has no role, `no role word`, `several roles:
commentary, programme` or `undecided: c012 (menu-off.tsv)` (the ids, joined with `, `, of the texts the net catches
that no decision fits: without a line, or with a stale one); then `the stream plain Play selects` (a commentary or
audio description with a strong tie on Play's own stream, MR-17); then `tie <word>: <reason>` (a tie that is not
strong, with its reason, Ties). It is empty when the role is set and the tie strong and the row is not a commentary
or audio description on Play's own stream; a `programme` row then still confirms nothing (The rule). Made up: c011
`AUDIO: Commentary by Person A` and c012 `COMMENTARY: Off` both label stream 4; without a decision on c012 the stream
has no role (`undecided: c012 (menu-off.tsv)`); with the line `off<TAB>c012<TAB>COMMENTARY: Off` its role is
`commentary`; with a line written for c012's earlier wording, `off<TAB>c012<TAB>COMMENTARY: Switch Off`, the line is
stale and the stream again has no role (`undecided: c012 (menu-off.tsv)`).

## The off-decisions file

`menu-off.tsv` in the map's folder (beside `menu-map.json`; for a map given as a file, in that file's folder) holds one
decision per choice that the net catches (MR-6). It is written from the choice's wording alone, as the measurement's
decisions file was (design 5.3); lazymkv only reads it (`cmd::menu::read_off`, `core::menu::parse_off`), and
`menu off` lists the lines it would need (Exit codes). Without the file there is no decision, and decide still runs.

- The file is UTF-8, read strictly; a leading byte-order mark is dropped. Lines are split on `\n`, a trailing `\r` left
  off, so a file saved with `\r\n` line ends reads the same.
- A blank line (one that holds only white space), and a line that starts with `#`, is skipped.
- Every other line is, since 2026-11-24, three fields split by one tab, `<decision><TAB><choice id><TAB><text>` (EM-3;
  engine menu maps design part 9): `off` (the choice states no role for the stream it leaves playing, so its words are
  not read) or `role` (its words are read as any other choice's); a choice id, `c` and three digits; and the choice's
  text as the role reading reads it (`role_text`, Roles), the wording the decision was made from, not empty.
- A line that repeats an earlier one (the same decision on the same id with the same text) is dropped.

**A decision applies only while it fits** (`OffDecisions::fits`): it is applied to a map only when the map has an
audio choice with that id whose text equals the line's text, code point for code point. The file is so bound to the
wording it was written for, not to the map file: a map saved again with a note added or a level raised keeps its
decisions, and a decision read with another map than its own decides nothing it was not written for.
- **A stale line**: the map has an audio choice with that id and its text differs from the line's. The line is not
  applied, the choice is read as having no decision (so a text the net catches leaves its stream without a role,
  MR-6), and its id is listed under `off_stale` in the `roles` key. `decide --menu` reads on; `menu publish` refuses
  the line (EO-12, EM-19).
- **An unused line**: one that is not stale and names no audio choice of the map, or one whose text fits and holds no
  role word. Its id is listed under `off_unused`; the two lists never share an id. It acts on nothing and is no refusal
  anywhere (EO-13, EM-22).

Made up (the fields are separated by one tab):

```
# c012 reads "COMMENTARY: Off" and switches the commentary off
off	c012	COMMENTARY: Off
role	c013	AUDIO: Commentary Without Subtitles
```

Its refusals, one sentence per problem in line order, each printed as `lazymkv-dev decide: error: <path>:
<problem>` (exit 2, no document), and the same by `menu off` and `menu publish` as `lazymkv-dev menu: error:
<path>: <problem>`; a line can give two:
- `line <n>: expected "off<TAB><choice id><TAB><the choice's text>" or "role<TAB><choice id><TAB><the choice's
  text>"` (not three fields split by one tab, or an empty text; a line of two fields, the form before 2026-11-24, is
  refused so, EM-3);
- `line <n>: unknown decision '<word>' (use "off" or "role")`;
- `line <n>: '<id>' is not a choice id (c and three digits)`;
- `line <n>: <id> is decided role here and off on line <m>` (or `off here and role`): one choice decided both ways;
- `line <n>: <id> has another text here than on line <m>`: one choice given two different texts.

The quoted word and id are written as Python's `repr` writes them. In the first sentence `<TAB>` is written as the five
characters `<TAB>`, not as a tab. A stale or unused line is no refusal of the file.

## Playlists

The menu role ties a choice to its stream through the recording's own Blu-ray playlists (`core::menu` `mpls.rs`,
`cmd::menu::read_playlists`; design 5.4a "Strong tie"). The order tie is still M3-1's; the playlist checks it (Ties).

- **What is read.** `decide --menu` reads, from the recording's `nav/BDMV/PLAYLIST/<name>.mpls`, the playlist of the
  label's title (its scan source file without `.mpls`) and the playlist of Play's effect and of every audio choice's
  effect, as the map spells them. Only a name of five ASCII digits is looked up on disk (MR-13); any other is `not a
  playlist name`. A name with no file is `no such file`.
- **What a playlist gives** (`parse_playlist`, the layout of the spike's `tools/bd.py` `parse_mpls`): its play items
  (each item's clip and its in and out time, 45 kHz ticks) and the first item's primary audio and subtitle stream
  tables, each entry with its PID, its coding type and its language as the disc writes it (three bytes; empty for a
  coding with no language field). It also says whether every play item carries those same two tables. A file that is
  not a playlist is refused with one sentence: `not a playlist: the file does not start with MPLS`, `the playlist is
  cut short at byte <n>` (the start of the read that failed), `the playlist has no play item`, `play item <i>: a stream
  entry of unknown type <t>` or `play item <i>: a stream entry without a coding type`, where `<i>` counts the play
  items from 0. No input makes the reader panic.
- **The title's playlist** (MR-8). Without it, or when its play items carry differing tables, no stream of the title
  has a tie: the `roles` key does not act, its `audio` and `unplaced` are empty, its `play` is null (since
  2026-11-24; `off_unused` and `off_stale` are still filled), and its `why` is one of `the title has
  no playlist`, `playlist <p> was not read`, `playlist <p>: <why>` (one of the reasons above, or the file's read
  error) and `playlist <p>: its play items carry differing stream tables`. `<p>` is printed as `menu check` prints
  playlists.

Made up: a recording with no `nav/BDMV/PLAYLIST/00800.mpls` gives the title of `00800.mpls` the `roles` key
`{"acts": false, "why": "playlist 00800: no such file", "audio": [], "unplaced": [], "off_unused": [], "off_stale": [],
"play": null}`.

## Ties

How firmly an audio choice is tied to its stream (`core::menu` `tie.rs`; design 5.3 "The tie", 5.4a "Strong tie").
The five words are `replay`, `attr`, `default`, `none` and `tie-conflict`; only `replay` and `attr` are strong.

**Placing a choice.** An audio choice reaches a number of the title's playlist (its audio table, from 1):
- on the title's own playlist (ASCII case ignored), its audio number, which must name an entry of the table;
- on a twin, its number translated by PID. A twin is another playlist that was read, whose play items all carry the
  same tables, and whose play items that the title's playlist also has (the same clip, in and out time, each counted
  once) cover at least 9/10 of the twin's ticks, which are more than 0. The twin's entry translates to the one entry of
  the title's playlist with the same PID, the same language and the same coding type (`map_forms.py` `match_streams`).

That number names the scan stream M3-1 names for it (the n-th audio stream without `CORE_AUDIO`). A choice placed
nowhere is listed under `unplaced` with why: `no effect`; `other playlist <p>` (not the title's playlist and not a
twin); `no stream number`; `unmatched: no audio <n> in <p>`; `unmatched: no audio <n> in the menu playlist`;
`unmatched: pid not in <p> (menu <q> audio <n>)`; `unmatched: attributes differ in <p> (menu <q> audio <n>)`;
`unmatched: pid twice in <p> (menu <q> audio <n>)`; `no scan stream for audio <k>`.

**A choice's tie**, in this order:
- `none`: the choice is below level `replayed` (why: `level <level>`);
- `replay`: plain Play's effect names a playlist, and the choice's playlist or audio number differs from Play's,
  compared exactly as the map writes them. `replay` holds when a choice's playlist differs from Play's even if its
  translated stream is the one Play selects: that is the measurement's rule unchanged (design 5.4a); such a stream's
  row is marked `play` and confirms nothing (MR-17, below);
- `attr`: the choice's text names exactly one language (the measurement's closed table of 24 languages), names no
  conflicting codecs, and exactly one entry of the title's playlist has that language (and one of the named codecs, if
  any), the one the choice selects;
- `default`: any other replayed choice, with `attr`'s reason (`attr: <n> languages named`, `attr: codecs conflict`,
  `attr: names another stream`, `attr: <n> streams fit`).

**A stream's tie** is the weakest of the ties of the choices that label it (from strong to weak `replay`, `attr`,
`default`, `none`), or `tie-conflict` (MR-9) when the playlist's entry and the scan stream M3-1 names for it disagree:
- the title's playlist lists another number of audio streams than the scan title has without `CORE_AUDIO` (then every
  labelled stream of the title): `the playlist lists <n> audio streams, the scan <m>`;
- their languages differ, both known: `language <entry> vs <scan>`. A language is compared lower-cased with the
  bibliographic codes folded (`fre` is `fra`); an empty one, `und`, `zxx`, `mul`, `mis` and `-` are unknown. A field
  that is not a code (zero bytes, spaces) counts as a known language, so against a known scan language it gives
  `tie-conflict`, the cautious direction;
- the coding type does not allow the scan stream's codec family: `codec 0x<cc> vs <codec>`. 0x80 allows LPCM, 0x81
  AC-3, 0x82 DTS, 0x83 TrueHD, 0x84 E-AC-3, 0x85 DTS-HD HR or DTS, 0x86 DTS-HD MA or DTS; any other coding allows every
  family.

**Play's own stream** (MR-17). Plain Play's effect is placed as an audio choice's effect is placed (on the title's
playlist, or through a twin by PID, with the same checks) and names the scan stream M3-1 names for that number. The
row of that stream has `play` true and never confirms, whatever its role and tie; when it would otherwise confirm,
its `why` is `the stream plain Play selects`. When Play has no effect, names no audio number, is on another playlist
or cannot be translated, Play's stream is not known and every row has `play` false. The ties are not changed by this:
a twin choice that lands on Play's stream is still `replay`, as the measurement has it.

`tie-conflict` is not strong whatever the choices' ties. Made up: the title's playlist lists audio 1 `eng` TrueHD,
audio 2 `eng` AC-3 and audio 3 `fra` AC-3; the scan's third audio stream without `CORE_AUDIO` is English AC-3; a
replayed commentary choice on audio 3 gives stream `tie-conflict` with why `language fra vs eng`, and confirms nothing.

## The rule

A menu map's commentary or audio-description role with a strong tie counts as a confirmation: the track is dropped,
whatever the content rule says of it and also without a verdict (owner, 2026-11-21 and 2026-11-24; MR-1, MA-1;
design 5.4a), under the guards below, and the decision says so in a note (since 2026-11-24; EO-2 to EO-5, EO-11). It
is built into `core::tracks::audio::decide_audio`; `decide --menu` gives it the confirmations and Play's stream.

**The confirmations** are the rows of the `roles` key, when it acts (MR-5, MR-8), whose role is `commentary` or
`audio_description`, whose tie is strong and that are not the stream plain Play itself selects (`play`, MR-17;
`StreamRole::confirms`). A `programme` role confirms nothing. A limit, stated plainly: when two choices label one
stream, one with a plain label that holds no role word and one that names a commentary, the stream reads as a
commentary (the measurement's reading: a text without a role word is skipped). Each confirmation carries the ids of
the choices that label the stream and, since 2026-11-24, their shown labels (`Choice::shown_label`, in the same order;
`MenuConfirmation`), which its note quotes. With them the decision is given the `roles` key's `play`, the scan stream
plain Play selects or none (`AudioFacts::menu_play`, EO-3). Without `--menu`, or with a block that does not act or a
`menu` key that is skipped, the decision is given none and is the one without the map.

**The drop.** After step 1 and steps 2 to 6 and "Content" as without the menu, each confirmation gets one of eight
outcomes (`MenuOutcome`; MA-3, and since 2026-11-24 EO-3 and EO-11). The first that holds of these three, tested in
this order:

1. `not_kept`: the stream is not among the kept tracks (step 1 dropped it, or the title has no such audio stream);
2. `overridden`: a remembered or owner's Keep override wins over the menu (MR-3);
3. `reference`: the stream is the decision's programme reference, which is never dropped. The decision adds the review
   reason `audio_reference_unclear` and the note `menu_reference_conflict` naming the stream (EO-5).

A confirmation none of them holds for is a candidate, whatever its language group (a stream without a language code
included). The content rule's verdict is not asked (MA-1): keep, commentary, cross-language commentary, a noted
commentary or audio description, the audio-description pattern, not judged, reference unclear, D21's veto holding,
verdicts measured against another reference than the decision's, or no verdict at all; none of them keeps a candidate.
Then every candidate gets the same outcome, the first that holds of these five, tested in this order (EP-8):

4. `reference_disputed`: a confirmation of the title has the outcome `reference`, so the map and lazymkv disagree
   about the film's own audio and none is dropped (EO-11). A confirmation on a reference that has a Keep override is
   `overridden` and disputes nothing.
5. `no_reference`: the first arrangement has no programme reference (every kept audio track has no language code, or
   `und`), and Play's stream is not known, or is not among the kept tracks, or is itself confirmed as a candidate, so
   none is dropped (EO-3, EM-10; since 2026-11-24, Ruling FR-1). Candidates are dropped only when Play's stream is
   known, is among the kept tracks and is not itself a candidate. On such a title Play's stream stands in for the
   reference: it is never a candidate (MR-17), so it stays.
6. `would_leave_none`: the kept streams without the candidates, arranged again by the same steps 2 to 6 and
   "Content", keep no track, so none is dropped (MA-2). It can no longer be reached, through `decide --menu` or by a
   direct call; it stays as a guard (Ruling FR-1, 2026-11-24);
7. `reference_would_change`: that arrangement's programme reference differs from the first arrangement's, so none is
   dropped (MR-4). The decision adds the review reason `audio_reference_unclear` and one note
   `menu_reference_conflict` naming no stream (EO-5);
8. `dropped`: else that arrangement is the decision, and every candidate is dropped as `AudioDrop::Menu`, with the
   note `menu_dropped` naming it (EO-2).

The kept streams are arranged again only when neither `reference_disputed` nor `no_reference` holds.
`would_leave_none` stays as a guard, but it can no longer be reached, through `decide --menu` or by a direct call
(EP-7; amended on 2026-11-24, Ruling FR-1): with a reference the reference stays, and without one nothing is dropped
unless Play's own stream is kept and is not itself a candidate, so it stays. A candidate that the wiring never gives,
on Play's own stream and handed to `decide_audio` directly, gets `no_reference`.

**The review.** A menu drop raises no review, as a flag or a description raises none (MR-11's "no review" stands), so a
job with a menu drop and nothing else to hold it files. Only `reference` and `reference_would_change` add a review
reason, `audio_reference_unclear` (EO-5). And the menu's drop never removes `audio_reference_unclear` (EO-4): when
the first arrangement raised it, the decision keeps it after the drop, so a `reference-unclear` verdict on a stream
the menu drops still holds the job. Otherwise the content rule's verdicts apply to the tracks that remain as they do
on any title: a dropped stream's `commentary_detected` goes with it (the owner's answer "Yes, drop it"), and a
commentary verdict on another kept track still sends the job to review.

**The notes** (since 2026-11-24; EO-2, EO-5; replacing MR-11's "a menu drop raises no note"). They follow the audio
decision's own notes, ascending stream, the note naming no stream last; `<role>` is `a commentary` or `an audio
description`, and `<labels>` the confirmation's shown labels in the choices' order, each in double quotes, joined with
`, `:
- `menu_dropped`, for each `dropped` stream `<n>`: `the disc's menu calls stream <n> <role> (<labels>); dropped on the
  menu's word`;
- `menu_reference_conflict`, for each `reference` stream `<n>`: `the disc's menu calls stream <n>, the programme
  reference, <role> (<labels>); nothing was dropped on the menu's word`;
- `menu_reference_conflict`, naming no stream, once for `reference_would_change`: `dropping the tracks the menu
  confirms (<streams>) would change the programme reference from <a> to <b>; none dropped`, `<streams>` being
  `stream 3` for one candidate and `streams 3, 4` for more, and `<a>` or `<b>` `none` for a reference that would be
  lost.

A double quote inside a label is not escaped. A label holding a control character or a line or paragraph separator
(U+2028, U+2029), which the writer keeps out of a label but not out of a section, is written as Python's `repr` writes
it, in place of the double quotes, so the detail stays one line. A confirmation with no labels writes no ` (...)` (it
does not happen through `decide --menu`). The other outcomes raise no note (EM-9).

The decision lists every confirmation it was given with its outcome (MR-11), as `{"stream", "role", "choices",
"outcome"}` and no content verdict (MA-3). When none is dropped, the decision is the one without the menu, the
confirmations listed, with the review reason and the note of `reference` and `reference_would_change` added. Step 1,
D13's duplicates and the confidence are not changed by the menu (MR-14). Audio only: subtitles are not touched.

Made up: stream 3 is confirmed as a commentary by c002 (`AUDIO: Commentary by Person A`), is kept, has no Keep
override and is not the decision's reference, stream 1. Whether the content rule's verdict on stream 3 is `keep`,
`commentary` or missing, stream 3 is dropped, listed as `{"stream": 3, "role": "commentary", "choices": ["c002"],
"outcome": "dropped"}`, and noted `the disc's menu calls stream 3 a commentary ("AUDIO: Commentary by Person A");
dropped on the menu's word`. Had c005 (`AUDIO: Commentary by Person B`) confirmed stream 1 too, a stream plain Play
does not select, stream 1 would be `reference`, stream 3 `reference_disputed`, nothing would be dropped, the job would
be held with `audio_reference_unclear`, and the note would read `the disc's menu calls stream 1, the programme
reference, a commentary ("AUDIO: Commentary by Person B"); nothing was dropped on the menu's word`. On a title whose
only kept audio streams have no language code (so no reference), with both confirmed, neither is dropped and both are
listed with the outcome `no_reference`: Play's stream, which would stand in for the reference, cannot be either of
them (MR-17), so it is not known or not kept; handed to `decide_audio` directly with Play's stream one of them, both
would still be `no_reference` (Ruling FR-1). With a third kept stream without a language code that plain Play
selects, both would be dropped, with a `menu_dropped` note each.

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

The role reading (Roles) gives the measurement's answer on every case of the shared role cases
(`menu-role-cases.json`). Outside them lazymkv's reading and the measurement's Python differ here, as the code reads
(added on 2026-11-24):
- Case. lazymkv lower-cases with Rust's `to_lowercase`, not with Python's case-insensitive match or `casefold`. They
  differ on characters such as U+0130 and the long s (U+017F): Rust lower-cases U+0130 to `i` and U+0307, so
  `VİSUALLY IMPAIRED` holds no role word in lazymkv, and Python's case-insensitive match treats a long s as an `s`, so
  `DEſCRIBED` names an audio description in Python and nothing in lazymkv.
- Word edges. A word character is `char::is_alphanumeric()` (Unicode's Alphabetic or Numeric) or `_`, where Python's
  word edge uses `str.isalnum()`. They differ on marks that are Alphabetic but not letters, such as U+0345: it is a
  word character in lazymkv and not in Python.
- White space. lazymkv's white space is Unicode's `White_Space` (Rust's `char::is_whitespace`) and the four separators
  U+001C to U+001F, which Python's `str.isspace` also counts. On the Python 3.9 the measurement ran on the two sets
  are equal; a later Unicode version in either language could make them differ. The off-decisions file finds a blank
  line with Rust's `trim`, which is `White_Space` alone.

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

Added on 2026-11-24: M3-1's "no playlist parser" no longer holds for the menu role. The order tie is still M3-1's; the
recording's playlist checks it (MR-8, MR-9; Playlists, Ties). `menu check` and the block's other keys parse no
playlist, as before.

The owner's two rulings on the menu role, quoted:
- 2026-11-21: asked "Rule that a menu role with a strong tie outranks a content verdict of keep?", the owner answered
  "Yes, rule so" (design 5.4a).
- 2026-11-24: asked "A menu map says a track is a commentary or audio description (strong tie) and lazymkv's content
  check kept it as programme. What should lazymkv do with that track?", the owner chose "Drop it as confirmed" over
  "Send the job to review".

The menu-role increment's rulings (the plan of 2026-11-24, `docs/superpowers/plans/2026-11-24-menu-role-increment.md`),
verbatim. MR-1 records the owner's answer; MR-2 to MR-16 are rulings of the plan that the owner can correct:

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
  - Added on 2026-11-24: MR-11's "a menu drop raises no note" is replaced by the owner's answer EO-2 (the note
    `menu_dropped`; The rule); its "no review" stands.
- MR-12: a track that the menu and the content rule both call a commentary or an audio description is unruled and
  stays as today (review or note, nothing dropped).
- MR-13: only a playlist name of five ASCII digits is read from disk.
- MR-14: a `duplicate` drop may name a stream that the menu then drops.
- MR-15: the `menu` key's `other_playlists` still counts a twin playlist's choices (M3-2 stands); the new `roles`
  key holds them translated to the title's streams.
- MR-16: acceptance runs on copies of the 13 recordings without their stub `tmdb.json` (with it decide has no plan
  and so no decision), so the plan's identity is unknown there and English is the only wanted language.

Added on 2026-11-24 at the final review of the branch (a ruling the owner can correct):

- MR-17: a stream that plain Play itself selects is never confirmed. Play's own stream is the scan stream Play's
  effect names, placed as an audio choice's effect is placed (on the title's playlist, or through a twin by PID).
  When Play has no effect, names no audio number, is on another playlist or cannot be translated, Play's stream is
  not known and this ruling excludes nothing. The ties stay the measurement's.

Added on 2026-11-24, after pull request #35 merged: the owner's rulings after pull request #35, quoted:
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

The menu-alone plan's rulings (the plan of 2026-11-24, `docs/superpowers/plans/2026-11-24-menu-alone.md`), verbatim;
they are rulings of the plan that the owner can correct:

- MA-1: the two answers together mean the content rule's verdict does not gate the menu's drop in any state: keep,
  commentary, cross-language commentary, a noted commentary or audio description, the audio-description pattern,
  not judged, reference unclear, D21's veto holding, another reference than the measured one, or no verdict at all.
  This replaces MR-2's verdict conditions and MR-12.
- MA-2: a drop that would leave the title with no kept audio track is not made: none of the title's confirmed streams
  is dropped (outcome `would_leave_none`). Now that a track with no language code can be dropped, a title without a
  programme reference could otherwise lose its only audio.
- MA-3: the decision's list of confirmations no longer carries a content verdict; the outcomes are `dropped`,
  `not_kept`, `overridden`, `reference`, `reference_would_change`, `would_leave_none`.
  - Added on 2026-11-24: the outcomes are now eight; the owner's answers EO-11 and EO-3 add `reference_disputed`
    and `no_reference`, tested in EP-8's order (The rule).
- MA-4: the six sentences that pull request #35 added to the main design are replaced by ones that say the rule as it
  now is; they stay dated 2026-11-24.

MA-1 replaces MR-2's verdict conditions and MR-12, and MR-11's list of outcomes is now MA-3's. Of MA-4's six sentences,
five are replaced; the sixth, in the risk "Commentary on Blu-ray and UHD", is true as it stands and is kept. MR-2's
condition that the stream be in the Original or English group falls with the owner's third answer (a track with no
language code is treated like any other); its other conditions, that the stream is kept and is not the reference, stand,
and so do the other rulings as written above.

Added on 2026-11-24: the rulings that pull request 1 of the engine menu maps design builds
(`docs/superpowers/specs/2026-11-24-engine-menu-maps-design.md`). The owner's answers, quoted as the design's part 3
quotes them:

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
- **EO-11, the other tracks when the menu contradicts the reference (was EM-18).** Asked "The menu calls lazymkv's
  reference track a commentary, so the job is held for review (your ruling). The same map also confirms OTHER tracks
  of that title as commentaries. What happens to those other tracks?", the owner chose "Drop nothing there
  (Recommended)" over "Still drop the others (as written)". The chosen option read: "The map and lazymkv disagree
  about the film's own audio, so nothing is dropped on the menu's word on that title; the job is held and you decide."
- **EO-12, a stale off line at publish (EM-19).** Asked "An off-decision line whose wording no longer matches the map
  (a 'stale' line) is ignored by the engine. What should the publish command on the Mac do when it sees one?", the
  owner chose "Refuse to publish (Recommended, as written)" over "Publish with a warning".
- **EO-13, a leftover off line (EM-22).** Asked "An off-decision line whose choice id is not in the map at all (a
  leftover from an older map) can act on nothing. What should happen to it?", the owner chose "Allow it, list as
  unused (Recommended, as written)" over "Refuse to publish".

The designer's calls that pull request 1 builds, verbatim from the design's part 17 (the owner saw each, and
confirmed EM-19 and EM-22):

- **EM-3.** An off line needs all three fields; a two-field line is refused. Cost: the 13 files are rewritten once.
- **EM-9.** An outcome that drops nothing raises no note, except the two of part 13.3. Cost: an override, a
  `no_reference` keep and the like are seen only in `decide.json`.
- **EM-10.** "Play's track is known" also requires that track to be among the kept ones. Cost: on an odd title a
  commentary stays.
- **EM-19** (confirmed by the owner, EO-12). `menu publish` refuses a stale off line, while the engine only ignores
  one. Cost: none known; a pair placed by hand is still read safely.
- **EM-22** (confirmed by the owner, EO-13). An off line whose id names no audio choice of the map is unused, not
  stale, and is refused nowhere. Cost: a leftover line from an older map is carried along until someone removes it; it
  acts on nothing.

The plan's rulings (the plan of 2026-11-24, `docs/superpowers/plans/2026-11-24-engine-menu-maps-pr1.md`), verbatim;
they are rulings of the plan that the owner can correct:

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
  - Amended at the final review (2026-11-24, Ruling FR-1): `decide_audio` itself refuses to drop the stream Play
    selects on a title without a reference, so the guard cannot be reached by a direct call either, and its two tests
    show `no_reference`.
- EP-8: the title-level outcomes are tested in this order: `reference_disputed`, `no_reference`, `would_leave_none`,
  `reference_would_change`, else `dropped`.
- EP-9: the acceptance scripts read the owner's private data and are not committed. The repository gets counts only.

What these change in the rulings above: MR-11's "no note" falls with EO-2 and its "no review" stands; MA-3's six
outcomes are eight (EO-3, EO-11); MR-6's off-decisions are still per choice, and now also bound to the choice's
wording (EM-3; The off-decisions file); MA-2 stands as a guard that `decide --menu` no longer reaches (EP-7). The
guards the owner kept stand as written. The engine still gives no map; the design's other answers and calls are not
quoted here.

## Tests and acceptance

Recorded on 2026-11-21 for the branch that adds the reader, `menu show`, `menu check` and `decide --menu`; the test
count again on 2026-11-22, after the pull request's review fixes (IDs and free text quoted where they would break a
printed line, `other_playlists` grouped with ASCII case ignored, an empty map path shown as the file name).

- Tests: `cargo test --workspace --exclude lazymkv-ocr --locked --offline` (CI's macOS command, run here with
  `--offline`), run on this Mac with toolchain 1.100.0: 3384 passed, 0 failed, 5 ignored, in 57 result lines, with no
  warning. 48 tests print `skipped:` on this Mac because the evidence clone is absent; CI's counts differ per system.
  `main` before the branch gave 3327 passed with the same command, so the branch adds 57 tests: 29 in `core::menu`, 19
  in the `menu` command, 8 in decide and 1 in lazymkvctl. Nothing has run on Windows or Linux yet; that is CI's part
  once the branch is pushed.
- Acceptance on the 13 real maps, each against its recording's scan and `meta.json` (counts only): `menu check` exits 0
  on 13 of 13 and says `identity: disc_id matches` on 13 of 13; `menu show` exits 0 on 13 of 13. Summed over the maps:
  entries 340, on_scan 244, not_in_scan 96 (on 7 maps), no_effect 0, warnings 0, failed 0.
- The two shared fixtures are byte-identical to the writer's copies.

### The menu-role increment (2026-11-24)

Recorded on 2026-11-24 for the branch that adds the playlist reader, the roles, the off-decisions, the ties and the
rule, and gives `decide --menu` the `roles` key and the decision's confirmations. Its counts, and its outcome words
(`content_not_keep`, `no_verdict`), are of the rule as pull request #35 built it, before the owner's rulings after
that merge widened it (Rulings, MA-1 to MA-4).

- Tests: `cargo test --workspace --exclude lazymkv-ocr --locked --offline --no-fail-fast`, run on this Mac with
  toolchain 1.100.0: 3506 tests in 58 result lines, 3505 passed, 1 failed, 5 ignored, with no warning. The one failure
  is `a_killed_worker_with_a_torn_last_record_counts_only_whole_records` in the engine's test binary
  `engine_file_phase`, one of its worker tests with a 2.0 s time cap: they fail on this Mac when it is busy, on `main`
  too (the branch's starting point gave 3430 passed and 4 failed, in the same binary, on the same day), and the binary
  passes when run alone, 94 of 94. `main` e94057a has 3434 tests with the same command, so the branch adds 72: 73 new
  ones (26 in `menu::tie`, 15 in `tracks::audio`, 12 in `menu::mpls`, 10 in `menu::role`, 8 in decide, 2 in the `menu`
  command) and one of decide's replaced. 48 tests print `skipped:` because the evidence clone is absent. Nothing has run
  on Windows or Linux yet; that is CI's part once the branch is pushed.
- Acceptance on the 13 real maps (counts only), `decide` without and with `--menu` on a copy of each recording (MR-16:
  twelve copies without their stub `tmdb.json`, so that decide has a plan; the identity is unknown there and English is
  the only wanted language): the exit codes are equal on 13 of 13; every key but `decision` and `menu` is byte-equal on
  13 of 13; `roles.acts` is true on 13 of 13; plain Play's own stream is found on 13 of 13 and no commentary or
  audio-description role sits on it (MR-17).
- The off-decisions: 126 audio choices, 68 with a role word. 14 of the measurement's 15 decisions were carried over, one
  line per choice (8 `off`, 6 `role`); the fifteenth is worded as the hand-walked disc's hand form, so one text of that
  map is undecided.
- The roles: 79 `roles.audio` rows. 73 have the role and the tie that the measurement's join gave the same stream. The
  other 6 carry no role: 2 are on the hand-walked disc, where the measurement used the hand form (ties `osd`); 4 are
  rows the join called `tie-conflict` because it compared the playlist's entry with the rip's file track, where lazymkv
  compares it with the scan stream (MR-9), which agrees with the playlist. 1 audio choice is unplaced.
- The rule: 30 confirmations. 10 are `dropped`, on 5 maps, the programme reference the same before and after on each; 12
  are `content_not_keep` (the content rule itself says commentary or notes one: MR-12); 8 are `no_verdict`. Of the
  measurement's 14 disagreeing rows 10 drop. The other 4 are on three maps whose rip fails the verify report's track
  match, so decide takes no content verdict on any stream there; the rule was not changed to make them drop (design
  5.4a).

### Menu alone (2026-11-24)

Recorded on 2026-11-24 for the branch that takes the content rule's verdict out of the menu's drop (the owner's rulings
after pull request #35; MA-1 to MA-4).

- Tests: `cargo test --workspace --exclude lazymkv-ocr --locked --offline --no-fail-fast`, run on this Mac with
  toolchain 1.100.0: 3509 tests in 58 result lines. One run of the final code gave 3509 passed, 0 failed, 5 ignored,
  with no warning. A second run gave 3508 passed and 1 failed: the engine's unit test
  `owner::tests::a_rip_result_that_cannot_be_persisted_fails_the_job_and_gives_the_token_back`, which fails now and then
  on this Mac and passed when the engine's unit tests ran alone (262 of 262). The branch changes no file of the engine.
  `main` 9f9e471 has 3506 tests with the same command, so the branch adds 3 and rewrites five of pull request #35's for
  the rule as it now is. Nothing has run on Windows or Linux yet; that is CI's part once the branch is pushed.
- Acceptance on the 13 real maps (counts only), on the same copies as the record above: the exit codes are equal on 13
  of 13; every key but `decision` and `menu` is byte-equal on 13 of 13; the `roles` are those of the record above (79
  rows, 73 with the measurement's role and tie, plain Play's own stream found on 13 of 13).
- The rule: 30 confirmations, 30 `dropped`, on 11 maps. The programme reference is the same before and after on each of
  the 11. `commentary_detected` leaves the review reasons on 4 maps, where every stream whose verdict raised it is now
  dropped; no other review reason changes on any map. No confirmation met a guard: none is the reference, has a Keep
  override, or is a title's last audio track.

### Engine menu maps, pull request 1 (2026-11-24)

Recorded on 2026-11-24 for the branch that binds an off-decision to the wording of its choice, gives the `roles` block
`off_stale` and `play`, adds the menu's two notes and the reference rules to the rule, and adds `menu off` and
`menu publish` (EP-1 to EP-9; Rulings FR-1 and FR-2 of its final review).

- Tests: `cargo test --workspace --exclude lazymkv-ocr --locked --offline --no-fail-fast`, run on this Mac with
  toolchain 1.100.0 and two test threads: 3537 passed, 0 failed, 5 ignored, in 58 result lines, with no warning. 48
  tests print `skipped:` because the evidence clone is absent. `main` 8afcd97 has 3509 tests with the same command, so
  the branch adds 28: 9 in the `menu` command, 7 in `tracks::audio`, 3 in `menu::role`, 3 in `menu::tie`, 3 in decide, 2
  in `review` and 1 in `core::menu`'s own tests. The run is of the final code; the commit that adds this record changes
  this document only. Nothing has run on Windows or Linux yet; that is CI's part once the branch is pushed.
- Acceptance on the 13 real maps (counts only), with a release build of the final code, on the same copies as the
  records above. It was run twice, before and after the final review's fixes, with the same counts.
- The off-decisions: the 14 carried decisions were written again in the three-field form, each with the text `menu off`
  prints for its id (8 `off`, 6 `role`). `menu off` then lists no `stale` line and 1 `undecided` text, the one the
  record above left undecided. `off_stale` and `off_unused` are empty on 13 of 13.
- Publishing: `menu publish` into a local folder standing in for the maps folder exits 0 on 13 of 13. The 13 folders
  are named by 40 upper-case hex digits; 5 hold a map and an off-decisions file and 8, whose source has no decision, a
  map alone, and each published file is byte-equal to its source. The maps were not published to the real maps folder;
  that follows at the owner's word.
- `decide` without and with `--menu`, the map taken from its published folder: the exit codes are equal on 13 of 13;
  every key but `decision` and `menu` is byte-equal on 13 of 13; `roles.acts` is true and `roles.play` is known on 13
  of 13; 79 `roles.audio` rows.
- The rule: 30 confirmations, 30 `dropped`, on 11 maps, as in the record above. The programme reference is the same
  before and after on each of the 11. There are 30 `menu_dropped` notes, one for each drop, and no
  `menu_reference_conflict`. `commentary_detected` leaves the review reasons on 4 maps and no other review reason
  changes. No confirmation met a guard: there is no `reference`, `reference_disputed`, `no_reference`,
  `reference_would_change`, `overridden` or `not_kept` on any map, so the reference rules are shown by the tests alone.
- Not part of this record: the engine does not read a map yet (the design's second pull request), and nothing ran on
  the Windows machine.
