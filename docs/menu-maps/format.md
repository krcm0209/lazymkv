# Menu map format v1

A menu map is one JSON file per disc that lists the choices of the disc's menu: what the menu calls each choice, and
which playlist and streams choosing it selects. It is made outside lazymkv, by the map writer (`menumap`, Python) and
its replay, and lazymkv reads it with `core::menu` (`read`, `validate`). This document is the format both sides keep
to. The design behind it is the menu maps design of 2026-11-17, `docs/superpowers/specs/2026-11-17-menu-maps-design.md`,
sections 3, 5.1, 5.3, 5.4 and 5.4a. Every example here is made up.

## What a map is and is not

- A map says only: the menu calls this choice X, and choosing it selects stream N of playlist P. It never says what a
  track *is*.
- A choice that could not be confirmed is kept, with its level and a note. Nothing is left out silently and nothing is
  rounded up.
- A map holds disc titles and readings of the disc's own text, so maps live outside every repository. Repository
  files name a disc by its slug only.
- Making a map never needs the owner to type or photograph a menu.
- A map changes one decision of lazymkv, and only where `lazymkv-dev decide --menu` gives it (the engine gives no
  map): an audio track the menu calls a commentary or an audio description, with a strong tie, on a map whose identity
  matches, is dropped where the content rule kept it (The rule; design 5.4a). Every other label is evidence shown
  beside the decisions.

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

A refusal prints `lazymkv-dev menu: error: <line>` on stderr, one line for each problem of the map, as
`<path>: <problem>`. A scan that cannot be typed or lists no title, or a `meta.json` that cannot be read, is one such
line too (a scan's refusal quoted and escaped as Python's `repr` writes it when the scan text it names would break the
line); a scan file that cannot be read prints the system's error text, which names its path. A usage error (a
missing, extra or unknown argument) prints a usage line first, then one line `lazymkv-dev menu: error: <message>`,
`lazymkv-dev menu show: error: <message>` or `lazymkv-dev menu check: error: <message>`, as argparse words it.

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
    - `off_unused`: the ids of the off-decisions that name no audio choice whose text holds a role word, ascending.

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
 "off_unused": []
}
```

The `decision` key gains two keys, each written only when it is not empty, so a decision with no confirmation prints as
before:
- `decision.facts.menu_streams`: the scan streams the block confirms as a commentary or an audio description (the
  ones the decision is given; `FactsSummary.menu_streams`), ascending;
- `decision.decision.audio.menu`: every confirmation the decision was given, ascending stream, with what became of it
  (`AudioDecision.menu`, MR-11), as `{"stream", "role", "choices", "outcome"}`, and `verdict`, the content rule's
  verdict, only with the outcome `content_not_keep` (The rule).

A stream the menu drops is listed under `decision.decision.audio.dropped` as
`[<stream>, {"why": "menu", "role": "<role>"}]` (`AudioDrop::Menu`), in scan order with the other drops. Made up, for
the block above with the content rule's verdict `keep` on stream 3:

```json
"dropped": [[3, {"why": "menu", "role": "commentary"}]],
"menu": [{"stream": 3, "role": "commentary", "choices": ["c002"], "outcome": "dropped"}]
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
  without a decision; any other text is read. Any undecided text leaves the stream without a role, whatever the other
  texts say (MR-6). Else the one distinct role read is the stream's role; none, or several different ones, give no
  role.

A stream's `why` in the `roles` key says why the row confirms nothing. Its forms, in the order the code tests them:
the conflict texts of a `tie-conflict` (Ties); then why the stream has no role, `no role word`, `several roles:
commentary, programme` or `undecided: c012 (menu-off.tsv)`; then `the stream plain Play selects` (a commentary or
audio description with a strong tie on Play's own stream, MR-17); then `tie <word>: <reason>` (a tie that is not
strong, with its reason, Ties). It is empty when the role is set and the tie strong and the row is not a commentary
or audio description on Play's own stream; a `programme` row then still confirms nothing (The rule). Made up: c011
`AUDIO: Commentary by Person A` and c012 `COMMENTARY: Off` both label stream 4; without a decision on c012 the stream
has no role (`undecided: c012 (menu-off.tsv)`); with `off` on c012 its role is `commentary`.

## The off-decisions file

`menu-off.tsv` in the map's folder (beside `menu-map.json`; for a map given as a file, in that file's folder) holds one
decision per choice that the net catches (MR-6). It is written from the choice's wording alone, as the measurement's
decisions file was (design 5.3); lazymkv only reads it (`cmd::menu::read_off`, `core::menu::parse_off`).
Without the file there is no decision, and decide still runs.

- The file is UTF-8, read strictly; a leading byte-order mark is dropped. Lines are split on `\n`, a trailing `\r` left
  off.
- A blank line, and a line that starts with `#`, is skipped.
- Every other line is `<decision><TAB><choice id>`: `off` (the choice states no role for the stream it leaves playing,
  so its words are not read) or `role` (its words are read as any other choice's), and a choice id, `c` and three
  digits.
- A line that repeats an earlier one is dropped.

`menu-off.tsv` names choices by id and is not bound to the map it was written for; a map that is made again with other
ids needs its decisions written again (a decision for an id that no longer bears a role word is listed under
`off_unused`).

Made up (the fields are separated by one tab):

```
# c012 reads "COMMENTARY: Off" and switches the commentary off
off	c012
role	c013
```

Its refusals, one sentence per problem in line order, each printed as `lazymkv-dev decide: error: <path>:
<problem>` (exit 2, no document); a line can give two:
- `line <n>: expected "off<TAB><choice id>" or "role<TAB><choice id>"` (not two fields split by one tab);
- `line <n>: unknown decision '<word>' (use "off" or "role")`;
- `line <n>: '<id>' is not a choice id (c and three digits)`;
- `line <n>: <id> is decided role here and off on line <m>` (or `off here and role`): one choice decided both ways.

The quoted word and id are written as Python's `repr` writes them. A decision on an id that names no audio choice whose
text holds a role word is no refusal: the `roles` key lists it under `off_unused`.

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
  has a tie: the `roles` key does not act, its `audio` and `unplaced` are empty, and its `why` is one of `the title has
  no playlist`, `playlist <p> was not read`, `playlist <p>: <why>` (one of the reasons above, or the file's read
  error) and `playlist <p>: its play items carry differing stream tables`. `<p>` is printed as `menu check` prints
  playlists.

Made up: a recording with no `nav/BDMV/PLAYLIST/00800.mpls` gives the title of `00800.mpls` the `roles` key
`{"acts": false, "why": "playlist 00800: no such file", "audio": [], "unplaced": [], "off_unused": []}`.

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

A menu map's commentary or audio-description role with a strong tie outranks the content rule's keep and counts as a
confirmation: the track is dropped (owner, 2026-11-21 and 2026-11-24; MR-1; design 5.4a). It is built into
`core::tracks::audio::decide_audio`; `decide --menu` gives it the confirmations.

**The confirmations** are the rows of the `roles` key, when it acts (MR-5, MR-8), whose role is `commentary` or
`audio_description`, whose tie is strong and that are not the stream plain Play itself selects (`play`, MR-17;
`StreamRole::confirms`). A `programme` role confirms nothing. A limit, stated plainly: when two choices label one
stream, one with a plain label that holds no role word and one that names a commentary, the stream reads as a
commentary (the measurement's reading: a text without a role word is skipped). Without `--menu`, or with a block
that does not act or a `menu` key that is skipped, the decision is given none and is the one without the map.

**The drop.** After step 1 and steps 2 to 6 and "Content" as without the menu, each confirmation gets the first
outcome that holds, tested in this order (`MenuOutcome`):

1. `not_kept`: the stream is not among the kept tracks (step 1 dropped it, or the title has no such audio stream);
2. `overridden`: a remembered or owner's Keep override wins over the menu (MR-3);
3. `no_language`: the stream is kept in the no-language group, which the content rule does not judge;
4. `reference`: the stream is the decision's programme reference, which is never dropped (MR-2);
5. `no_verdict`: the verdicts were not measured (no reference they were measured against);
6. `verdicts_not_applied`: the verdicts were measured against another reference than the decision's, or D21's veto
   holds (MR-2);
7. `no_verdict`: no verdict names the stream;
8. `content_not_keep`: the content rule's verdict on the stream is not `keep` (MR-2, MR-12), written under `verdict`;
9. else the stream is a candidate. The kept streams without the candidates are arranged again by the same steps 2 to 6
   and "Content". When that keeps the programme reference, it is the decision and every candidate is `dropped` (as
   `AudioDrop::Menu`); otherwise none is dropped and every candidate is `reference_would_change` (MR-4).

A menu drop raises no note and no review, as a flag or a description raises none; the decision lists every
confirmation it was given with its outcome (MR-11). Step 1, D13's duplicates and the confidence are not changed by the
menu (MR-14). A track that the menu and the content rule both call a commentary or an audio description is unruled
and stays as today: a commentary verdict still sends the job to review, and nothing is dropped (`content_not_keep`,
MR-12). Audio only: subtitles are not touched.

Made up: stream 3 is confirmed as a commentary and the content rule's verdict on it, measured against the decision's
reference stream 1, is `keep`; stream 3 is dropped. With the verdict `commentary` it is kept and listed as
`{"stream": 3, "role": "commentary", "choices": ["c002"], "outcome": "content_not_keep", "verdict": "commentary"}`.

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
rule, and gives `decide --menu` the `roles` key and the decision's confirmations.

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
