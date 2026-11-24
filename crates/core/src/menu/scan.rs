//! Menu maps against a scan (menu maps design 5.3 and 5.4, rulings M3-1 to M3-6 of `docs/menu-maps/format.md`):
//! which scan stream a map's stream number means, whether a map belongs to the scanned disc, the check `menu check`
//! prints, and the per-title block of labels decide shows beside its decisions. Evidence only: nothing here changes a
//! verdict, role, plan or decision. No I/O.

use crate::disc::{Disc, Flags, StreamKind, Title};

use super::{map_level, Effect, Kind, Level, MenuMap, RolesBlock};

/// Why a disc's ID is unknown: a scan file was given, not the recording folder (M3-6).
pub const NO_ID_IN_SCAN_FILE: &str = "a scan file carries no disc ID; give the recording folder";
/// Why a disc's ID is unknown: the recording folder has no `meta.json` (M3-6).
pub const NO_META: &str = "the recording has no meta.json";
/// Why a disc's ID is unknown: the recording's `meta.json` has no `disc_id` (M3-6).
pub const META_WITHOUT_ID: &str = "the recording's meta.json has no disc_id";
/// Why the identity is not checked: the map has no `disc.disc_id_sha1` (M3-6).
pub const MAP_WITHOUT_ID: &str = "the map has no disc_id_sha1";

/// The scan streams a title's map numbers name, as scan stream numbers in scan order (M3-1): map audio n is
/// `audio[n - 1]`, the n-th audio stream without `CORE_AUDIO`; map subtitle n is `subtitles[n - 1]`, the n-th subtitle
/// stream without `DERIVED`. MakeMKV's observed order (it held for all 244 effects on a scan title of the 13 maps), not
/// a spec; no playlist is parsed.
#[derive(Clone, Debug, PartialEq)]
pub struct MapStreams {
    pub audio: Vec<u32>,
    pub subtitles: Vec<u32>,
}

/// The title's `MapStreams` (M3-1).
pub fn map_streams(title: &Title) -> MapStreams {
    let numbers = |kind: StreamKind, skipped: u32| -> Vec<u32> {
        let counted = title.streams.iter().filter(|stream| stream.kind == kind && !stream.flags.has(skipped));
        counted.map(|stream| stream.number).collect()
    };
    MapStreams {
        audio: numbers(StreamKind::Audio, Flags::CORE_AUDIO),
        subtitles: numbers(StreamKind::Subtitles, Flags::DERIVED),
    }
}

/// The scan stream that map number `n` names among `numbers` (one kind's `MapStreams` list); None below 1 or above the
/// count.
fn stream_of(numbers: &[u32], n: i64) -> Option<u32> {
    usize::try_from(n).ok().filter(|&n| n >= 1).and_then(|n| numbers.get(n - 1)).copied()
}

/// True when the title's source file is `<playlist>.mpls`, ASCII case ignored (design 5.4).
fn plays(title: &Title, playlist: &str) -> bool {
    title.source_file.as_deref().map_or(false, |file| file.eq_ignore_ascii_case(&format!("{playlist}.mpls")))
}

/// A map's playlist as a printed line shows it: five ASCII digits as they are, any other text quoted and escaped as
/// Python's `repr` does (`py_repr_str`), so that a map's playlist cannot break a line or forge one. Print side only:
/// the writer's rules take any string.
pub fn playlist_text(playlist: &str) -> String {
    if playlist.len() == 5 && playlist.bytes().all(|byte| byte.is_ascii_digit()) {
        playlist.to_string()
    } else {
        crate::recording::py_repr_str(playlist)
    }
}

/// A disc ID or a digest (a map's `disc_id_sha1` or `listing_sig`, a recording's `disc_id`) as a printed line shows
/// it: ASCII hex digits as they are, any other text (an empty one too) quoted and escaped as `playlist_text` does, so
/// that neither a map nor a `meta.json` can break a line or forge one. Print side only: the writer's rules and the
/// identity's comparison take any string.
pub fn id_text(id: &str) -> String {
    if !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        id.to_string()
    } else {
        crate::recording::py_repr_str(id)
    }
}

/// True for a character that breaks a printed line, the characters the writer keeps out of a label: a control
/// character (Cc, C1 controls such as U+0085 included) or a line or paragraph separator (U+2028, U+2029).
pub(super) fn breaks_a_line(c: char) -> bool {
    c.is_control() || c == '\u{2028}' || c == '\u{2029}'
}

/// A map's free text (a section, a label, a note, a script, a `made` value) as a printed line shows it: as it is, or
/// quoted and escaped (`py_repr_str`) when it holds a character that `breaks_a_line`, so that it cannot break a line
/// or forge one.
pub fn line_text(text: &str) -> String {
    if text.chars().any(breaks_a_line) {
        crate::recording::py_repr_str(text)
    } else {
        text.to_string()
    }
}

/// The titles whose source file is `<playlist>.mpls`, ASCII case ignored, in title order: every title an effect on
/// `playlist` plays (two titles may share one source file). Empty for a playlist that is no title of the scan (M3-2).
pub fn titles_for<'a>(disc: &'a Disc, playlist: &str) -> Vec<&'a Title> {
    disc.titles.values().filter(|title| plays(title, playlist)).collect()
}

/// The scanned disc's ID (a recording's `meta.json` `disc_id`), or the reason it is not known (M3-6).
#[derive(Clone, Debug, PartialEq)]
pub enum DiscId {
    Known(String),
    Absent(String),
}

/// Whether a map belongs to the scanned disc, by `disc_id_sha1` against the disc's ID only (M3-6; `listing_sig` is
/// shown and not checked). `Matches` holds the map's ID; `NotChecked` the reason.
#[derive(Clone, Debug, PartialEq)]
pub enum Identity {
    Matches(String),
    Differs { map: String, disc: String },
    NotChecked(String),
}

impl Identity {
    /// The identity as `menu check` and decide's block print it: `disc_id matches (<id>)`, `disc_id differs: the map's
    /// is <a>, the disc's is <b>` or `not checked: <reason>`, each ID as `id_text` shows it.
    pub fn text(&self) -> String {
        match self {
            Identity::Matches(id) => format!("disc_id matches ({})", id_text(id)),
            Identity::Differs { map, disc } => {
                format!("disc_id differs: the map's is {}, the disc's is {}", id_text(map), id_text(disc))
            }
            Identity::NotChecked(reason) => format!("not checked: {reason}"),
        }
    }
}

/// The map's `disc_id_sha1` against the disc's ID, ASCII case ignored (M3-6). The map's ID is read as a recording's
/// is: surrounding white space left off, and an empty one is no ID. A map without an ID is not checked whatever the
/// disc's (`MAP_WITHOUT_ID`: no folder the person could give would let it be checked); otherwise a disc without a
/// known ID is not checked for its own reason.
pub fn identity(map: &MenuMap, disc: &DiscId) -> Identity {
    let Some(ours) = map.disc.disc_id_sha1.as_deref().map(str::trim).filter(|id| !id.is_empty()) else {
        return Identity::NotChecked(MAP_WITHOUT_ID.to_string());
    };
    match disc {
        DiscId::Absent(reason) => Identity::NotChecked(reason.clone()),
        DiscId::Known(theirs) if theirs.eq_ignore_ascii_case(ours) => Identity::Matches(ours.to_string()),
        DiscId::Known(theirs) => Identity::Differs { map: ours.to_string(), disc: theirs.clone() },
    }
}

/// What `menu check` found (design 5.4, M3-2 to M3-6): the identity, the lines it prints (ids, playlists and numbers
/// only, never a label; the last is the CHECK line) and the counts the CHECK line gives. `entries` counts play and
/// the choices; `on_scan` those whose playlist is a title of the scan, `not_in_scan` those whose playlist is not
/// (their streams are not checked, M3-2), `no_effect` those without an effect; `warnings` and `failed` count per
/// line, and `failed` adds 1 when the identity differs.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckReport {
    pub identity: Identity,
    pub lines: Vec<String>,
    pub entries: usize,
    pub on_scan: usize,
    pub not_in_scan: usize,
    pub no_effect: usize,
    pub warnings: usize,
    pub failed: usize,
}

/// What one part of an effect line adds to the counts.
enum Mark {
    Fine,
    Warning,
    Failure,
}

/// The audio part of an effect line on one title: `audio n = stream s`; `audio n OUT OF RANGE (k audio streams)` (a
/// failure) below 1 or above the count; `audio none`, with `, no audio stream named (warning)` when the entry is an
/// audio choice (M3-5).
fn audio_part(audio: Option<i64>, streams: &[u32], own_kind: bool) -> (String, Mark) {
    match audio {
        None if own_kind => ("audio none, no audio stream named (warning)".to_string(), Mark::Warning),
        None => ("audio none".to_string(), Mark::Fine),
        Some(n) => match stream_of(streams, n) {
            Some(stream) => (format!("audio {n} = stream {stream}"), Mark::Fine),
            None => (format!("audio {n} OUT OF RANGE ({} audio streams)", streams.len()), Mark::Failure),
        },
    }
}

/// The subtitle part of an effect line on one title. Shown: `subtitle n = stream s`; `subtitle n OUT OF RANGE (k
/// subtitle streams)` (a failure) below 1 or above the count; `subtitle none shown`, with `, no subtitle stream named
/// (warning)` when the entry is a subtitle choice (M3-5). Off, the number is a leftover that names no stream (M3-4):
/// `subtitle n off`; `subtitle n off, leftover beyond k (warning)` above the count and `subtitle n off, leftover below
/// 1 (warning)` below 1, never a failure; `subtitle none` without a number.
fn subtitle_part(effect: &Effect, streams: &[u32], own_kind: bool) -> (String, Mark) {
    let count = streams.len();
    match (effect.subtitle_shown, effect.subtitle) {
        (true, None) if own_kind => {
            ("subtitle none shown, no subtitle stream named (warning)".to_string(), Mark::Warning)
        }
        (true, None) => ("subtitle none shown".to_string(), Mark::Fine),
        (true, Some(n)) => match stream_of(streams, n) {
            Some(stream) => (format!("subtitle {n} = stream {stream}"), Mark::Fine),
            None => (format!("subtitle {n} OUT OF RANGE ({count} subtitle streams)"), Mark::Failure),
        },
        (false, None) => ("subtitle none".to_string(), Mark::Fine),
        (false, Some(n)) if n < 1 => (format!("subtitle {n} off, leftover below 1 (warning)"), Mark::Warning),
        (false, Some(n)) if stream_of(streams, n).is_none() => {
            (format!("subtitle {n} off, leftover beyond {count} (warning)"), Mark::Warning)
        }
        (false, Some(n)) => (format!("subtitle {n} off"), Mark::Fine),
    }
}

/// `menu check` of a map against a scanned disc (design 5.4, M3-2 to M3-6). The lines: `identity: <Identity::text>`;
/// `listing_sig: not checked (lazymkv does not recompute it)`; per entry, play first and then the choices in map
/// order, `<id>: no effect recorded`, `<id>: playlist <p> not in the scan`, or one line per title of the playlist
/// (`titles_for`), `<id>: playlist <p>, title <n>: <audio part>, <subtitle part>` (`<p>` is `playlist_text`), each
/// title checked on its own streams (`map_streams`); last `CHECK ok|FAILED entries=.. on_scan=.. not_in_scan=..
/// no_effect=.. warnings=.. failed=..; streams of not_in_scan entries were not checked`, FAILED when `failed` is above
/// 0. A missing number is a warning only on the choice's own kind (M3-5), never a failure.
pub fn check(map: &MenuMap, disc: &Disc, identity: Identity) -> CheckReport {
    let mut lines = vec![format!("identity: {}", identity.text())];
    let mut failed = usize::from(matches!(identity, Identity::Differs { .. }));
    lines.push("listing_sig: not checked (lazymkv does not recompute it)".to_string());
    let (mut on_scan, mut not_in_scan, mut no_effect, mut warnings) = (0, 0, 0, 0);
    let play = std::iter::once(("play", None, map.play.effect.as_ref()));
    let choices = map.choices.iter().map(|choice| (choice.id.as_str(), Some(choice.kind), choice.effect.as_ref()));
    let entries: Vec<(&str, Option<Kind>, Option<&Effect>)> = play.chain(choices).collect();
    for &(id, kind, effect) in &entries {
        let Some(effect) = effect else {
            lines.push(format!("{id}: no effect recorded"));
            no_effect += 1;
            continue;
        };
        let titles = titles_for(disc, &effect.playlist);
        let playlist = playlist_text(&effect.playlist);
        if titles.is_empty() {
            lines.push(format!("{id}: playlist {playlist} not in the scan"));
            not_in_scan += 1;
            continue;
        }
        on_scan += 1;
        for title in titles {
            let streams = map_streams(title);
            let audio = audio_part(effect.audio, &streams.audio, kind == Some(Kind::Audio));
            let subtitle = subtitle_part(effect, &streams.subtitles, kind == Some(Kind::Subtitle));
            for (_, mark) in [&audio, &subtitle] {
                match mark {
                    Mark::Fine => {}
                    Mark::Warning => warnings += 1,
                    Mark::Failure => failed += 1,
                }
            }
            let number = title.number;
            lines.push(format!("{id}: playlist {playlist}, title {number}: {}, {}", audio.0, subtitle.0));
        }
    }
    let verdict = if failed == 0 { "ok" } else { "FAILED" };
    lines.push(format!(
        "CHECK {verdict} entries={} on_scan={on_scan} not_in_scan={not_in_scan} no_effect={no_effect} \
         warnings={warnings} failed={failed}; streams of not_in_scan entries were not checked",
        entries.len()
    ));
    CheckReport { identity, lines, entries: entries.len(), on_scan, not_in_scan, no_effect, warnings, failed }
}

/// One label in decide's block (design 5.3, 5.4): the entry's id (`play` for play), what a person is shown
/// (`Choice::shown_label`; play's label alone), its check level, and whether its effect differs from plain Play as the
/// map says (None for play, and where the map has null).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MenuLabel {
    pub id: String,
    pub label: String,
    pub level: Level,
    pub differs_from_play: Option<bool>,
}

/// The labels of the choices that select one scan stream (its scan number), in map order (design 5.3).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StreamLabels {
    pub stream: u32,
    pub labels: Vec<MenuLabel>,
}

/// A playlist other than the title's that the map's effects name: how many entries name it, and whether it is a title
/// of the scan (design 5.4, M3-2).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OtherPlaylist {
    pub playlist: String,
    pub entries: u32,
    pub in_scan: bool,
}

/// Decide's `menu` block for one title (design 5.3, 5.4; evidence only, no decision reads it): the map's slug and
/// level, the identity's text, the title's playlist (its source file without `.mpls`; None without a source file),
/// what starts the title (play and the version, bonus and other choices on its playlist), the audio and shown
/// subtitle choices by the scan stream of their own number, the subtitle choices that turn subtitles off, every other
/// playlist the map's effects name, the own-kind choices whose own number labels no stream, and the entries without an
/// effect. `roles` is the menu-role increment's reading of the audio choices (design 5.4a, `tie.rs`); `title_block`
/// sets it to None, and None is not printed, so a block without it prints as before.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MenuBlock {
    pub map: String,
    pub map_level: Level,
    pub identity: String,
    pub playlist: Option<String>,
    pub starts: Vec<MenuLabel>,
    pub audio: Vec<StreamLabels>,
    pub subtitles: Vec<StreamLabels>,
    pub subtitles_off: Vec<MenuLabel>,
    pub other_playlists: Vec<OtherPlaylist>,
    pub out_of_range: Vec<String>,
    pub no_effect: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<RolesBlock>,
}

/// `file` without a final `.mpls` (ASCII case ignored); a source file of another kind is kept whole.
pub(super) fn playlist_of(file: &str) -> String {
    match file.len().checked_sub(5) {
        Some(at) if file.is_char_boundary(at) && file[at..].eq_ignore_ascii_case(".mpls") => file[..at].to_string(),
        _ => file.to_string(),
    }
}

/// Adds one entry on `playlist` to `others`, a new row in first-seen order. Playlists that differ in ASCII case alone
/// share one row, its first-seen spelling, as `titles_for` matches them to one source file.
fn count_other(others: &mut Vec<OtherPlaylist>, disc: &Disc, playlist: &str) {
    match others.iter_mut().find(|other| other.playlist.eq_ignore_ascii_case(playlist)) {
        Some(other) => other.entries += 1,
        None => others.push(OtherPlaylist {
            playlist: playlist.to_string(),
            entries: 1,
            in_scan: !titles_for(disc, playlist).is_empty(),
        }),
    }
}

/// Adds `label` to the row of `stream`.
fn label_stream(rows: &mut Vec<StreamLabels>, stream: u32, label: MenuLabel) {
    match rows.iter_mut().find(|row| row.stream == stream) {
        Some(row) => row.labels.push(label),
        None => rows.push(StreamLabels { stream, labels: vec![label] }),
    }
}

/// Decide's `menu` block for `title` of `disc` (design 5.3, 5.4, M3-2 to M3-5). Only a choice's own kind labels a
/// stream: an audio choice the audio stream of its audio number, a shown subtitle choice the subtitle stream of its
/// subtitle number. A side effect (the subtitle an audio choice turns on, any stream of a version, bonus or other
/// choice: M3-3) labels nothing. A subtitle choice with subtitles off goes under `subtitles_off` only (M3-4). An
/// own-kind number below 1, above the count or null goes under `out_of_range` (M3-5). Streams ascend, labels keep map
/// order; `other_playlists` groups every effect on another playlist (ASCII case ignored) in first-seen order, play
/// first.
pub fn title_block(map: &MenuMap, disc: &Disc, title: &Title, identity: &Identity) -> MenuBlock {
    let streams = map_streams(title);
    let mut block = MenuBlock {
        map: map.disc.slug.clone(),
        map_level: map_level(map),
        identity: identity.text(),
        playlist: title.source_file.as_deref().map(playlist_of),
        starts: Vec::new(),
        audio: Vec::new(),
        subtitles: Vec::new(),
        subtitles_off: Vec::new(),
        other_playlists: Vec::new(),
        out_of_range: Vec::new(),
        no_effect: Vec::new(),
        roles: None,
    };
    match &map.play.effect {
        None => block.no_effect.push("play".to_string()),
        Some(effect) if plays(title, &effect.playlist) => block.starts.push(MenuLabel {
            id: "play".to_string(),
            label: map.play.label.clone(),
            level: map.play.check.level,
            differs_from_play: None,
        }),
        Some(effect) => count_other(&mut block.other_playlists, disc, &effect.playlist),
    }
    for choice in &map.choices {
        let Some(effect) = &choice.effect else {
            block.no_effect.push(choice.id.clone());
            continue;
        };
        if !plays(title, &effect.playlist) {
            count_other(&mut block.other_playlists, disc, &effect.playlist);
            continue;
        }
        let label = MenuLabel {
            id: choice.id.clone(),
            label: choice.shown_label(),
            level: choice.check.level,
            differs_from_play: choice.differs_from_play,
        };
        match choice.kind {
            Kind::Audio => match effect.audio.and_then(|n| stream_of(&streams.audio, n)) {
                Some(stream) => label_stream(&mut block.audio, stream, label),
                None => block.out_of_range.push(choice.id.clone()),
            },
            Kind::Subtitle if !effect.subtitle_shown => block.subtitles_off.push(label),
            Kind::Subtitle => match effect.subtitle.and_then(|n| stream_of(&streams.subtitles, n)) {
                Some(stream) => label_stream(&mut block.subtitles, stream, label),
                None => block.out_of_range.push(choice.id.clone()),
            },
            Kind::Version | Kind::Bonus | Kind::Other => block.starts.push(label),
        }
    }
    block.audio.sort_by_key(|row| row.stream);
    block.subtitles.sort_by_key(|row| row.stream);
    block
}
