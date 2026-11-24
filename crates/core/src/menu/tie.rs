//! A map's audio choices tied to a title's scan streams, and each stream's role (menu maps design 5.4a "Strong tie" and
//! "Menu role"; rulings MR-6, MR-8 and MR-9 of the menu-role plan). A choice reaches the title's playlist (the block)
//! directly or through a twin playlist translated by PID, then the scan stream M3-1 names for that number; its tie is
//! the measurement's `replay`, `attr`, `default` or `none` (`map_forms.py`), and the playlist's entry is checked
//! against the scan stream (`tie-conflict`, MR-9). The language and codec names of a text port `map_forms.py`
//! exactly. Only a commentary or audio-description role with a strong tie confirms anything. No I/O: the playlists
//! come read.

use std::collections::BTreeMap;

use unicode_normalization::char::canonical_combining_class;
use unicode_normalization::UnicodeNormalization;

use crate::codec::Family;
use crate::disc::{Stream, StreamKind, Title};
use crate::lang::{norm_lang, NO_LANGUAGE};

use super::scan::playlist_of;
use super::{
    audio_roles_in, map_streams, playlist_text, role_text, stream_role, AudioRole, Choice, Effect, Identity, Kind,
    Level, MenuMap, OffDecisions, Playlist, PlaylistStream, RoleReading,
};

/// How firmly a choice is tied to its stream (design 5.3, 5.4a). serde and `word()` give "replay", "attr",
/// "default", "none", "tie-conflict".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tie {
    /// The choice is replayed and its playlist or audio number differs from plain Play's.
    Replay,
    /// A replayed choice whose text names one language (and its codec, if any) that only its own stream has.
    Attr,
    /// A replayed choice that neither `Replay` nor `Attr` fits.
    Default,
    /// A choice below level `replayed`.
    None,
    /// The playlist's entry and the scan stream disagree (MR-9); not strong whatever the choices' ties.
    TieConflict,
}

impl Tie {
    /// True for the strong ties of design 5.4a: `Replay` and `Attr`.
    pub fn is_strong(self) -> bool {
        matches!(self, Tie::Replay | Tie::Attr)
    }

    /// The tie's word (the serde spelling).
    pub fn word(self) -> &'static str {
        match self {
            Tie::Replay => "replay",
            Tie::Attr => "attr",
            Tie::Default => "default",
            Tie::None => "none",
            Tie::TieConflict => "tie-conflict",
        }
    }

    /// A choice's tie from strong (3) to weak (0); a stream has the weakest of its choices' ties.
    fn strength(self) -> u8 {
        match self {
            Tie::Replay => 3,
            Tie::Attr => 2,
            Tie::Default => 1,
            Tie::None | Tie::TieConflict => 0,
        }
    }
}

/// The measurement's languages (`map_forms.py`): an ISO 639-2/T code and its words, each word its own phrase.
const LANGUAGES: [(&str, &str); 24] = [
    ("eng", "english"), ("fra", "french francais canadien quebecois"), ("spa", "spanish espanol castellano latino"),
    ("deu", "german deutsch"), ("ita", "italian italiano"), ("por", "portuguese portugues brasileiro"),
    ("jpn", "japanese"), ("kor", "korean"), ("zho", "chinese mandarin cantonese"), ("nld", "dutch nederlands"),
    ("rus", "russian"), ("pol", "polish polski"), ("ces", "czech cesky cestina"), ("hun", "hungarian magyar"),
    ("tha", "thai"), ("tur", "turkish turkce"), ("swe", "swedish svenska"), ("dan", "danish dansk"),
    ("nor", "norwegian norsk"), ("fin", "finnish suomi"), ("ell", "greek"), ("heb", "hebrew"), ("ara", "arabic"),
    ("hin", "hindi"),
];

/// The measurement's codec phrases (`map_forms.py`) and the codec names each may mean.
const CODEC_PHRASES: [(&str, &[&str]); 16] = [
    ("dolby digital plus", &["EAC3"]), ("dd+", &["EAC3"]), ("e-ac-3", &["EAC3"]),
    ("dolby digital", &["AC3"]), ("ac-3", &["AC3"]), ("ac3", &["AC3"]),
    ("truehd", &["TrueHD"]), ("atmos", &["TrueHD", "EAC3"]),
    ("dts-hd master audio", &["DTS-HD MA"]), ("dts-hd ma", &["DTS-HD MA"]), ("dts-hd master", &["DTS-HD MA"]),
    ("dts:x", &["DTS-HD MA"]), ("dts-hd", &["DTS-HD HR", "DTS-HD MA"]), ("dts", &["DTS", "DTS-HD HR", "DTS-HD MA"]),
    ("lpcm", &["LPCM"]), ("pcm", &["LPCM"]),
];

/// `text` as the measurement matches it: NFKD, every character of a canonical combining class other than 0 removed,
/// lower-cased.
fn folded(text: &str) -> Vec<char> {
    let bare: String = text.nfkd().filter(|&c| canonical_combining_class(c) == 0).collect();
    bare.to_lowercase().chars().collect()
}

/// `text` with every match of `phrase` blanked to one space, scanning left to right as Python's `re.sub` does; a match
/// needs a neighbour on each side that is not alphanumeric (an underscore is an edge). The bool says whether any
/// matched.
fn blank(text: &[char], phrase: &[char]) -> (Vec<char>, bool) {
    let edge = |at: Option<&char>| !at.is_some_and(|c| c.is_alphanumeric());
    let (mut out, mut at, mut hit) = (Vec::with_capacity(text.len()), 0, false);
    while at < text.len() {
        let before = at.checked_sub(1).and_then(|i| text.get(i));
        if text[at..].starts_with(phrase) && edge(before) && edge(text.get(at + phrase.len())) {
            out.push(' ');
            at += phrase.len();
            hit = true;
        } else {
            out.push(text[at]);
            at += 1;
        }
    }
    (out, hit)
}

/// The values of the phrases of `table` that `text` names (`map_forms.py` `_named`): phrases tried longest first, equal
/// lengths in alphabetical order, every match of a found phrase blanked before the next is tried.
fn named<T: Copy>(text: &str, table: &[(&'static str, T)]) -> Vec<T> {
    let mut phrases: Vec<&(&str, T)> = table.iter().collect();
    phrases.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()).then(a.0.cmp(b.0)));
    let (mut text, mut found) = (folded(text), Vec::new());
    for &(phrase, value) in phrases {
        let phrase: Vec<char> = phrase.chars().collect();
        let (blanked, hit) = blank(&text, &phrase);
        if hit {
            found.push(value);
            text = blanked;
        }
    }
    found
}

/// The ISO 639-2/T codes `text` names, sorted (the measurement's closed table).
pub fn languages_named(text: &str) -> Vec<&'static str> {
    let table: Vec<(&'static str, &'static str)> =
        LANGUAGES.iter().flat_map(|&(code, words)| words.split(' ').map(move |word| (word, code))).collect();
    let mut codes = named(text, &table);
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// The codec names `text` names, sorted: Some(empty) when none is named, None when the named ones conflict.
pub fn codecs_named(text: &str) -> Option<Vec<&'static str>> {
    let found = named(text, &CODEC_PHRASES);
    let Some((first, rest)) = found.split_first() else {
        return Some(Vec::new());
    };
    let mut names: Vec<&'static str> =
        first.iter().copied().filter(|name| rest.iter().all(|names| names.contains(name))).collect();
    names.sort_unstable();
    names.dedup();
    if names.is_empty() {
        Option::None
    } else {
        Some(names)
    }
}

/// The measurement's name of a coding type: 0x80 "LPCM", 0x81 "AC3", 0x82 "DTS", 0x83 "TrueHD", 0x84 "EAC3",
/// 0x85 "DTS-HD HR", 0x86 "DTS-HD MA", 0xA1 "EAC3", 0xA2 "DTS-HD HR"; None for any other.
pub fn coding_name(coding: u8) -> Option<&'static str> {
    match coding {
        0x80 => Some("LPCM"),
        0x81 => Some("AC3"),
        0x82 => Some("DTS"),
        0x83 => Some("TrueHD"),
        0x84 | 0xA1 => Some("EAC3"),
        0x85 | 0xA2 => Some("DTS-HD HR"),
        0x86 => Some("DTS-HD MA"),
        _ => Option::None,
    }
}

/// True when a scan stream of `family` may carry a playlist entry of `coding` (MR-9): 0x80 LPCM; 0x81 AC-3; 0x82 DTS;
/// 0x83 TrueHD; 0x84 E-AC-3; 0x85 DTS-HD HR or DTS; 0x86 DTS-HD MA or DTS; any other coding allows every family.
fn allows(coding: u8, family: Family) -> bool {
    match coding {
        0x80 => family == Family::Lpcm,
        0x81 => family == Family::Ac3,
        0x82 => family == Family::Dts,
        0x83 => family == Family::TrueHd,
        0x84 => family == Family::Eac3,
        0x85 => matches!(family, Family::DtsHdHr | Family::Dts),
        0x86 => matches!(family, Family::DtsHdMa | Family::Dts),
        _ => true,
    }
}

/// A playlist entry's language as the comparisons read it: `norm_lang` (lower-cased, bibliographic folded).
fn entry_language(entry: &PlaylistStream) -> String {
    norm_lang(Some(&entry.language))
}

/// One scan audio stream that at least one audio choice labels.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StreamRole {
    /// The scan stream's number.
    pub stream: u32,
    /// The role its choices give it (`stream_role`), None without exactly one.
    pub role: Option<AudioRole>,
    /// The weakest of its choices' ties, or `TieConflict` (MR-9).
    pub tie: Tie,
    /// The ids of the choices that label it, in map order.
    pub choices: Vec<String>,
    /// Why it confirms nothing: empty when the role is Some and the tie strong.
    pub why: String,
}

impl StreamRole {
    /// The role the menu confirms: a commentary or an audio description with a strong tie (design 5.4a, MR-1); never
    /// a programme role, never a tie conflict.
    pub fn confirms(&self) -> Option<AudioRole> {
        match self.role {
            Some(role @ (AudioRole::Commentary | AudioRole::AudioDescription)) if self.tie.is_strong() => Some(role),
            _ => Option::None,
        }
    }
}

/// An audio choice that labels no stream of the title, and why.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Unplaced {
    /// The choice's id.
    pub id: String,
    /// Why it labels no stream.
    pub why: String,
}

/// What `audio_roles` found for one title: why no stream is tied (MR-8; None when the block was read), the labelled
/// streams ascending, the audio choices that label none in map order, and the off-decisions that name no audio choice
/// with a role word.
#[derive(Clone, Debug, PartialEq)]
pub struct RolesReport {
    pub untied: Option<String>,
    pub audio: Vec<StreamRole>,
    pub unplaced: Vec<Unplaced>,
    pub off_unused: Vec<String>,
}

/// The `roles` key of decide's menu block.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RolesBlock {
    /// True when the roles may act: the block was read and the map's identity matches (MR-5, MR-8).
    pub acts: bool,
    /// Why they may not act; empty when they may.
    pub why: String,
    /// `RolesReport::audio`.
    pub audio: Vec<StreamRole>,
    /// `RolesReport::unplaced`.
    pub unplaced: Vec<Unplaced>,
    /// `RolesReport::off_unused`.
    pub off_unused: Vec<String>,
}

impl RolesReport {
    /// The block decide prints: `acts` when nothing left the title untied and `identity` matches (MR-5); `why` is the
    /// untied reason, else `the map's identity is <Identity::text()>` when it does not match, else empty.
    pub fn block(self, identity: &Identity) -> RolesBlock {
        let matches = matches!(identity, Identity::Matches(_));
        let why = match (&self.untied, matches) {
            (Some(why), _) => why.clone(),
            (Option::None, false) => format!("the map's identity is {}", identity.text()),
            (Option::None, true) => String::new(),
        };
        RolesBlock {
            acts: self.untied.is_none() && matches,
            why,
            audio: self.audio,
            unplaced: self.unplaced,
            off_unused: self.off_unused,
        }
    }
}

/// The off-decision ids that name no audio choice of the map, or one whose `role_text` holds no role word; id order.
fn unused_decisions(map: &MenuMap, off: &OffDecisions) -> Vec<String> {
    let used = |id: &str| {
        map.choices.iter().any(|choice| {
            choice.kind == Kind::Audio
                && choice.id == id
                && !audio_roles_in(&role_text(&choice.section, &choice.label)).is_empty()
        })
    };
    off.ids().into_iter().filter(|id| !used(id)).map(str::to_string).collect()
}

/// The title's own playlist (the block) and its name, or why no stream of the title is tied (MR-8).
fn block_of<'a>(
    title: &Title,
    playlists: &'a BTreeMap<String, Result<Playlist, String>>,
) -> Result<(String, &'a Playlist), String> {
    let Some(file) = title.source_file.as_deref() else {
        return Err("the title has no playlist".to_string());
    };
    let name = playlist_of(file);
    let shown = playlist_text(&name);
    match playlists.get(&name) {
        Option::None => Err(format!("playlist {shown} was not read")),
        Some(Err(error)) => Err(format!("playlist {shown}: {error}")),
        Some(Ok(block)) if !block.uniform => {
            Err(format!("playlist {shown}: its play items carry differing stream tables"))
        }
        Some(Ok(block)) => Ok((name, block)),
    }
}

/// True when `menu` is a twin of `block`: the play items they share (same clip, in and out time; each block item
/// counted once) cover at least 9/10 of the menu playlist's ticks, which must be above 0 (design 5.4a).
fn is_twin(menu: &Playlist, block: &Playlist) -> bool {
    let mut left: BTreeMap<(&str, u32, u32), usize> = BTreeMap::new();
    for item in &block.items {
        *left.entry((item.clip.as_str(), item.in_time, item.out_time)).or_default() += 1;
    }
    let mut shared = 0u64;
    for item in &menu.items {
        let key = (item.clip.as_str(), item.in_time, item.out_time);
        if let Some(count) = left.get_mut(&key).filter(|count| **count > 0) {
            *count -= 1;
            shared += u64::from(item.out_time.saturating_sub(item.in_time));
        }
    }
    let ticks = menu.ticks();
    ticks > 0 && shared.saturating_mul(10) >= ticks.saturating_mul(9)
}

/// The block number of twin `menu`'s audio `n`: the one block entry with the same PID, language (`norm_lang`) and
/// coding (`map_forms.py` `match_streams`), or why there is none.
fn translate(n: i64, menu_name: &str, menu: &Playlist, block_name: &str, block: &Playlist) -> Result<usize, String> {
    let Some(entry) = usize::try_from(n).ok().filter(|&k| k >= 1).and_then(|k| menu.audio.get(k - 1)) else {
        return Err(format!("unmatched: no audio {n} in the menu playlist"));
    };
    let (block_shown, menu_shown) = (playlist_text(block_name), playlist_text(menu_name));
    let same_pid: Vec<usize> = (0..block.audio.len()).filter(|&i| block.audio[i].pid == entry.pid).collect();
    match same_pid[..] {
        [] => Err(format!("unmatched: pid not in {block_shown} (menu {menu_shown} audio {n})")),
        [i] if entry_language(&block.audio[i]) != entry_language(entry) || block.audio[i].coding != entry.coding => {
            Err(format!("unmatched: attributes differ in {block_shown} (menu {menu_shown} audio {n})"))
        }
        [i] => Ok(i + 1),
        _ => Err(format!("unmatched: pid twice in {block_shown} (menu {menu_shown} audio {n})")),
    }
}

/// The block number an audio choice's effect names (from 1, within the block's audio table), or why it names none.
fn block_number(
    effect: &Effect,
    block_name: &str,
    block: &Playlist,
    playlists: &BTreeMap<String, Result<Playlist, String>>,
) -> Result<usize, String> {
    let twin = if effect.playlist.eq_ignore_ascii_case(block_name) {
        Option::None
    } else {
        match playlists.get(&effect.playlist) {
            Some(Ok(menu)) if menu.uniform && is_twin(menu, block) => Some(menu),
            _ => return Err(format!("other playlist {}", playlist_text(&effect.playlist))),
        }
    };
    let Some(n) = effect.audio else {
        return Err("no stream number".to_string());
    };
    if let Some(menu) = twin {
        return translate(n, &effect.playlist, menu, block_name, block);
    }
    match usize::try_from(n).ok().filter(|&k| k >= 1 && k <= block.audio.len()) {
        Some(k) => Ok(k),
        Option::None => Err(format!("unmatched: no audio {n} in {}", playlist_text(block_name))),
    }
}

/// Whether a replayed choice's text fits its block entry `k` alone (the measurement's `attr`), or the reason it does
/// not.
fn attr(text: &str, k: usize, block: &Playlist) -> Result<(), String> {
    let languages = languages_named(text);
    let [language] = languages[..] else {
        return Err(format!("attr: {} languages named", languages.len()));
    };
    let Some(codecs) = codecs_named(text) else {
        return Err("attr: codecs conflict".to_string());
    };
    let fits: Vec<usize> = (0..block.audio.len())
        .filter(|&i| {
            let entry = &block.audio[i];
            entry_language(entry) == language
                && (codecs.is_empty() || coding_name(entry.coding).is_some_and(|name| codecs.contains(&name)))
        })
        .map(|i| i + 1)
        .collect();
    match fits[..] {
        [only] if only == k => Ok(()),
        [_] => Err("attr: names another stream".to_string()),
        _ => Err(format!("attr: {} streams fit", fits.len())),
    }
}

/// A choice's tie to its block entry `k` and the reason a tie is not strong (empty for a strong one): `None` below
/// level `replayed`; `Replay` when plain Play's effect names a playlist and the choice's playlist or audio number
/// differs from Play's, compared exactly; else `Attr`, or `Default` with `attr`'s reason.
fn tie_of(map: &MenuMap, choice: &Choice, effect: &Effect, k: usize, block: &Playlist) -> (Tie, String) {
    if choice.check.level < Level::Replayed {
        return (Tie::None, format!("level {}", choice.check.level.word()));
    }
    let replay = map.play.effect.as_ref().is_some_and(|play| {
        !play.playlist.is_empty() && (effect.playlist != play.playlist || effect.audio != play.audio)
    });
    if replay {
        return (Tie::Replay, String::new());
    }
    match attr(&role_text(&choice.section, &choice.label), k, block) {
        Ok(()) => (Tie::Attr, String::new()),
        Err(why) => (Tie::Default, why),
    }
}

/// Why block entry `k` and the scan stream it names disagree (MR-9), or None: the block lists another number of audio
/// streams than the scan's `scan_count`; the languages differ, both known; or the coding does not allow the stream's
/// codec family.
fn conflict(block: &Playlist, k: usize, stream: &Stream, scan_count: usize) -> Option<String> {
    if block.audio.len() != scan_count {
        return Some(format!("the playlist lists {} audio streams, the scan {scan_count}", block.audio.len()));
    }
    let entry = block.audio.get(k.checked_sub(1)?)?;
    let (ours, theirs) = (entry_language(entry), norm_lang(stream.language.as_deref()));
    let known = |code: &str| !NO_LANGUAGE.contains(&code);
    if known(&ours) && known(&theirs) && ours != theirs {
        return Some(format!("language {ours} vs {theirs}"));
    }
    if !allows(entry.coding, stream.codec.family) {
        return Some(format!("codec 0x{:02X} vs {}", entry.coding, stream.codec.display));
    }
    Option::None
}

/// One audio choice that labels a scan stream: its id, its text, its tie and the tie's reason.
struct Labelled<'a> {
    id: &'a str,
    text: String,
    tie: Tie,
    reason: String,
}

/// The roles of `title`'s audio streams from `map`'s audio choices (design 5.4a, MR-6, MR-8, MR-9). `playlists` holds
/// the recording's playlists by name without `.mpls`. In order: the unused off-decisions; the block, or `untied` with
/// nothing placed; each audio choice in map order placed on a block number (directly, or through a twin by PID) and
/// the scan stream M3-1 names for it, with its tie, or unplaced with the reason; then per labelled stream, ascending,
/// its choices, the weakest tie (`TieConflict` when the entry and the stream disagree), the role and why it confirms
/// nothing.
pub fn audio_roles(
    map: &MenuMap,
    off: &OffDecisions,
    title: &Title,
    playlists: &BTreeMap<String, Result<Playlist, String>>,
) -> RolesReport {
    let off_unused = unused_decisions(map, off);
    let (block_name, block) = match block_of(title, playlists) {
        Ok(found) => found,
        Err(why) => return RolesReport { untied: Some(why), audio: Vec::new(), unplaced: Vec::new(), off_unused },
    };
    let scan = map_streams(title).audio;
    let mut unplaced = Vec::new();
    let mut by_stream: BTreeMap<u32, (usize, &Stream, Vec<Labelled>)> = BTreeMap::new();
    for choice in map.choices.iter().filter(|choice| choice.kind == Kind::Audio) {
        let Some(effect) = &choice.effect else {
            unplaced.push(Unplaced { id: choice.id.clone(), why: "no effect".to_string() });
            continue;
        };
        let k = match block_number(effect, &block_name, block, playlists) {
            Ok(k) => k,
            Err(why) => {
                unplaced.push(Unplaced { id: choice.id.clone(), why });
                continue;
            }
        };
        let stream = scan.get(k - 1).and_then(|&number| {
            title.streams.iter().find(|stream| stream.number == number && stream.kind == StreamKind::Audio)
        });
        let Some(stream) = stream else {
            unplaced.push(Unplaced { id: choice.id.clone(), why: format!("no scan stream for audio {k}") });
            continue;
        };
        let (tie, reason) = tie_of(map, choice, effect, k, block);
        let text = role_text(&choice.section, &choice.label);
        let row = by_stream.entry(stream.number).or_insert_with(|| (k, stream, Vec::new()));
        row.2.push(Labelled { id: &choice.id, text, tie, reason });
    }
    let audio = by_stream
        .into_iter()
        .map(|(number, (k, stream, labelled))| stream_role_of(number, k, stream, &labelled, block, scan.len(), off))
        .collect();
    RolesReport { untied: Option::None, audio, unplaced, off_unused }
}

/// The `StreamRole` of scan stream `number` (block entry `k`) from the choices that label it, in map order.
fn stream_role_of(
    number: u32,
    k: usize,
    stream: &Stream,
    labelled: &[Labelled],
    block: &Playlist,
    scan_count: usize,
    off: &OffDecisions,
) -> StreamRole {
    let weakest = labelled.iter().min_by_key(|label| label.tie.strength());
    let texts: Vec<(&str, &str)> = labelled.iter().map(|label| (label.id, label.text.as_str())).collect();
    let reading = stream_role(&texts, off);
    let role = match reading {
        RoleReading::Role(role) => Some(role),
        _ => Option::None,
    };
    let clash = conflict(block, k, stream, scan_count);
    let tie = match (&clash, weakest) {
        (Some(_), _) => Tie::TieConflict,
        (Option::None, Some(label)) => label.tie,
        (Option::None, Option::None) => Tie::None,
    };
    let why = if role.is_some() && tie.is_strong() {
        String::new()
    } else if let Some(clash) = clash {
        clash
    } else {
        match reading {
            RoleReading::Silent => "no role word".to_string(),
            RoleReading::Several(roles) => {
                let words: Vec<&str> = roles.iter().map(|role| role.as_str()).collect();
                format!("several roles: {}", words.join(", "))
            }
            RoleReading::Undecided(ids) => format!("undecided: {} (menu-off.tsv)", ids.join(", ")),
            RoleReading::Role(_) => {
                let reason = weakest.map_or("", |label| label.reason.as_str());
                format!("tie {}: {reason}", tie.word())
            }
        }
    };
    StreamRole {
        stream: number,
        role,
        tie,
        choices: labelled.iter().map(|label| label.id.to_string()).collect(),
        why,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::disc::Title;
    use crate::menu::{
        parse_off, parse_playlist, title_block, Applies, Check, Choice, Effect, Kind, Level, MadeUpItem, MapDisc,
        MenuMap, OffDecisions, Play, Playlist, Start, Surface, FORMAT,
    };

    /// The playlists of a test, keyed by name without `.mpls`.
    type Playlists = BTreeMap<String, Result<Playlist, String>>;

    /// The block's audio table in most tests: TrueHD eng, then two AC-3 eng.
    const BLOCK_AUDIO: [(u16, u8, &str); 3] = [(0x1100, 0x83, "eng"), (0x1101, 0x81, "eng"), (0x1102, 0x81, "eng")];

    /// The scan audio of most tests: TrueHD eng (stream 1), its AC-3 core (2, flagged 256), AC-3 eng (3 and 4), so
    /// block audio n is scan stream 1, 3, 4.
    const SCAN_AUDIO: [(&str, &str, &str); 4] =
        [("eng", "TrueHD", "0"), ("eng", "AC3", "256"), ("eng", "AC3", "0"), ("eng", "AC3", "0")];

    /// A check at `level`.
    fn check(level: Level) -> Check {
        Check {
            label_reread: "same".to_string(),
            level,
            replays: 3,
            replays_equal: Some(true),
            player: false,
            note: String::new(),
        }
    }

    /// An effect on `playlist` with audio `audio` and subtitles off.
    fn effect(playlist: &str, audio: Option<i64>) -> Effect {
        Effect { playlist: playlist.to_string(), audio, subtitle: None, subtitle_shown: false }
    }

    /// A choice of `kind` in section `SETUP > <heading>` with `label`, `effect` and `level`.
    fn choice(id: &str, kind: Kind, heading: &str, label: &str, effect: Option<Effect>, level: Level) -> Choice {
        Choice {
            id: id.to_string(),
            section: format!("SETUP > {heading}"),
            label: label.to_string(),
            label_image: None,
            kind,
            surface: Surface::Main,
            script: format!("scripts/{id}.txt"),
            applies: Applies::AtPlay,
            effect,
            differs_from_play: None,
            check: check(level),
        }
    }

    /// An audio choice under `AUDIO` whose effect is `playlist` audio `audio`.
    fn audio(id: &str, label: &str, playlist: &str, number: i64, level: Level) -> Choice {
        choice(id, Kind::Audio, "AUDIO", label, Some(effect(playlist, Some(number))), level)
    }

    /// A made-up map with play's effect `play` and `choices`.
    fn map(play: Option<Effect>, choices: Vec<Choice>) -> MenuMap {
        MenuMap {
            format: FORMAT.to_string(),
            disc: MapDisc { slug: "made-up-1".to_string(), disc_id_sha1: None, listing_sig: None, format: None },
            made: json!({}),
            start: Start { script: None, passed: Vec::new(), seconds_to_menu: None },
            play: Play { label: "PLAY".to_string(), script: None, effect: play, check: check(Level::Replayed) },
            choices,
            not_mapped: Vec::new(),
            quirks: Vec::new(),
        }
    }

    /// Play on `00800` audio 1.
    fn play() -> Option<Effect> {
        Some(effect("00800", Some(1)))
    }

    /// A playlist of `items` (clip, in, out), each carrying `audio`.
    fn playlist(items: &[(&'static str, u32, u32)], audio: &[(u16, u8, &'static str)]) -> Playlist {
        let items: Vec<MadeUpItem> = items
            .iter()
            .map(|&(clip, in_time, out_time)| MadeUpItem {
                clip,
                in_time,
                out_time,
                audio: audio.to_vec(),
                subtitles: Vec::new(),
            })
            .collect();
        parse_playlist(&crate::menu::made_up_playlist(&items)).unwrap()
    }

    /// The playlists `entries`.
    fn playlists(entries: Vec<(&str, Result<Playlist, String>)>) -> Playlists {
        entries.into_iter().map(|(name, playlist)| (name.to_string(), playlist)).collect()
    }

    /// Only the block `00800`: one item, `BLOCK_AUDIO`.
    fn block_only() -> Playlists {
        playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO)))])
    }

    /// The scan title 0 on `source` with a video stream 0 and the audio streams `(language, codec, flags)` from 1.
    fn scan_title(source: &str, audio: &[(&str, &str, &str)]) -> Title {
        let mut text = format!("CINFO:1,0,\"Blu-ray disc\"\nTINFO:0,16,0,\"{source}\"\nSINFO:0,0,1,0,\"Video\"\n");
        for (index, &(language, codec, flags)) in audio.iter().enumerate() {
            let (id, short) = match codec {
                "TrueHD" => ("A_TRUEHD", "TrueHD"),
                "AC3" => ("A_AC3", "DD"),
                "DTS-HD MA" => ("A_DTS", "DTS-HD MA"),
                other => panic!("no test codec {other}"),
            };
            let attrs = [(1, "Audio"), (3, language), (5, id), (6, short), (7, short), (22, flags)];
            for (attr, value) in attrs {
                text.push_str(&format!("SINFO:0,{},{attr},0,\"{value}\"\n", index + 1));
            }
        }
        crate::disc::disc(&crate::robot::parse(&text)).unwrap().titles[&0].clone()
    }

    /// The title of most tests: `00800.mpls` with `SCAN_AUDIO`.
    fn title() -> Title {
        scan_title("00800.mpls", &SCAN_AUDIO)
    }

    /// `audio_roles` without off-decisions.
    fn roles(map: &MenuMap, title: &Title, playlists: &Playlists) -> RolesReport {
        audio_roles(map, &OffDecisions::default(), title, playlists)
    }

    /// An unplaced choice.
    fn unplaced(id: &str, why: &str) -> Unplaced {
        Unplaced { id: id.to_string(), why: why.to_string() }
    }

    /// The commentary choice `c010` on `00800` audio 3 at `level`.
    fn commentary(level: Level) -> Choice {
        audio("c010", "Commentary by Person A", "00800", 3, level)
    }

    /// Every `names` case: the measurement's languages and codecs of a text.
    #[test]
    fn the_shared_cases_give_the_measurements_languages_and_codecs() {
        let all: serde_json::Value = serde_json::from_str(crate::menu::ROLE_CASES).unwrap();
        let strings = |value: &serde_json::Value| -> Vec<String> {
            value.as_array().unwrap().iter().map(|item| item.as_str().unwrap().to_string()).collect()
        };
        let cases = all["names"].as_array().unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let text = case["text"].as_str().unwrap();
            assert_eq!(languages_named(text), strings(&case["languages"]), "{case}");
            let codecs = if case["codecs"].is_null() { None } else { Some(strings(&case["codecs"])) };
            let ours = codecs_named(text).map(|names| names.iter().map(|name| name.to_string()).collect::<Vec<_>>());
            assert_eq!(ours, codecs, "{case}");
        }
    }

    /// The coding names of the measurement, and none for another coding.
    #[test]
    fn a_coding_type_has_the_measurements_name() {
        let names = [
            (0x80, "LPCM"),
            (0x81, "AC3"),
            (0x82, "DTS"),
            (0x83, "TrueHD"),
            (0x84, "EAC3"),
            (0x85, "DTS-HD HR"),
            (0x86, "DTS-HD MA"),
            (0xA1, "EAC3"),
            (0xA2, "DTS-HD HR"),
        ];
        for (coding, name) in names {
            assert_eq!(coding_name(coding), Some(name), "{coding:#x}");
        }
        assert_eq!(coding_name(0x90), None);
        assert_eq!(coding_name(0), None);
    }

    /// The five spellings, through `word` and serde both ways, and which are strong.
    #[test]
    fn a_tie_spells_as_serde_writes_it() {
        let spellings = [
            (Tie::Replay, "replay", true),
            (Tie::Attr, "attr", true),
            (Tie::Default, "default", false),
            (Tie::None, "none", false),
            (Tie::TieConflict, "tie-conflict", false),
        ];
        for (tie, word, strong) in spellings {
            assert_eq!(tie.word(), word);
            assert_eq!(tie.is_strong(), strong, "{word}");
            assert_eq!(serde_json::to_string(&tie).unwrap(), format!("\"{word}\""));
            assert_eq!(serde_json::from_str::<Tie>(&format!("\"{word}\"")).unwrap(), tie);
        }
    }

    /// A replayed commentary choice on another audio number than Play's is a `replay` tie and confirms the commentary.
    #[test]
    fn a_replayed_choice_that_differs_from_play_has_the_replay_tie() {
        let report = roles(&map(play(), vec![commentary(Level::Replayed)]), &title(), &block_only());
        assert_eq!(report.untied, Option::None);
        assert_eq!(report.unplaced, vec![]);
        assert_eq!(report.audio.len(), 1);
        let role = &report.audio[0];
        assert_eq!(role.stream, 4);
        assert_eq!(role.role, Some(AudioRole::Commentary));
        assert_eq!(role.tie, Tie::Replay);
        assert_eq!(role.choices, vec!["c010".to_string()]);
        assert_eq!(role.why, "");
        assert_eq!(role.confirms(), Some(AudioRole::Commentary));
    }

    /// A choice at level `checked` or `read` has tie `none` and confirms nothing.
    #[test]
    fn a_choice_below_replayed_has_no_tie() {
        for (level, why) in [(Level::Checked, "tie none: level checked"), (Level::Read, "tie none: level read")] {
            let report = roles(&map(play(), vec![commentary(level)]), &title(), &block_only());
            let role = &report.audio[0];
            assert_eq!((role.role, role.tie, role.why.as_str()), (Some(AudioRole::Commentary), Tie::None, why));
            assert_eq!(role.confirms(), Option::None);
        }
    }

    /// A replayed choice equal to Play is `attr` only when its text's one language and codec fit its own entry alone;
    /// otherwise `default` with the measurement's reason.
    #[test]
    fn a_choice_equal_to_play_is_attr_when_its_language_and_codec_fit_one_stream_and_default_otherwise() {
        let block_audio = [(0x1100, 0x83, "eng"), (0x1101, 0x81, "fra"), (0x1102, 0x81, "eng")];
        let lists = playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &block_audio)))]);
        let scan = scan_title(
            "00800.mpls",
            &[("eng", "TrueHD", "0"), ("eng", "AC3", "256"), ("fra", "AC3", "0"), ("eng", "AC3", "0")],
        );
        let cases = [
            ("Main English Dolby TrueHD", Tie::Attr, ""),
            ("Main English/French", Tie::Default, "tie default: attr: 2 languages named"),
            ("Main English", Tie::Default, "tie default: attr: 2 streams fit"),
            ("Main French", Tie::Default, "tie default: attr: names another stream"),
            ("Main English TrueHD DTS", Tie::Default, "tie default: attr: codecs conflict"),
            ("Main Feature", Tie::Default, "tie default: attr: 0 languages named"),
        ];
        for (label, tie, why) in cases {
            let report = roles(&map(play(), vec![audio("c001", label, "00800", 1, Level::Replayed)]), &scan, &lists);
            assert_eq!(report.audio.len(), 1, "{label}");
            let role = &report.audio[0];
            assert_eq!((role.stream, role.role), (1, Some(AudioRole::Programme)), "{label}");
            assert_eq!((role.tie, role.why.as_str()), (tie, why), "{label}");
            assert_eq!(role.confirms(), Option::None, "{label}");
        }
    }

    /// Without Play's effect, or with one that names no playlist, nothing is a `replay` tie.
    #[test]
    fn without_a_play_effect_there_is_no_replay_tie() {
        for play in [Option::None, Some(effect("", Some(1)))] {
            let report = roles(&map(play.clone(), vec![commentary(Level::Replayed)]), &title(), &block_only());
            let role = &report.audio[0];
            let want = (Tie::Default, "tie default: attr: 0 languages named");
            assert_eq!((role.tie, role.why.as_str()), want, "{play:?}");
            assert_eq!(role.confirms(), Option::None);
        }
    }

    /// A stream labelled by a replayed and a read choice has tie `none`, the read choice's reason.
    #[test]
    fn several_choices_on_one_stream_have_their_weakest_tie() {
        let read = audio("c011", "Commentary", "00800", 3, Level::Read);
        let report = roles(&map(play(), vec![commentary(Level::Replayed), read]), &title(), &block_only());
        assert_eq!(report.audio.len(), 1);
        let role = &report.audio[0];
        assert_eq!(role.choices, vec!["c010".to_string(), "c011".to_string()]);
        assert_eq!((role.tie, role.why.as_str()), (Tie::None, "tie none: level read"));
        assert_eq!(role.confirms(), Option::None);
    }

    /// A `replay` and a `default` choice on one stream (the second through a twin) leave it `default`: nothing
    /// confirmed.
    #[test]
    fn a_default_and_a_replay_choice_on_one_stream_do_not_confirm() {
        let twin_audio = [(0x1100, 0x83, "eng"), (0x1102, 0x81, "eng")];
        let lists = playlists(vec![
            ("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO))),
            ("00801", Ok(playlist(&[("00001", 0, 900)], &twin_audio))),
        ]);
        let replay = commentary(Level::Replayed);
        let default = audio("c011", "Commentary", "00801", 2, Level::Replayed);
        let report = roles(&map(Some(effect("00801", Some(2))), vec![replay, default]), &title(), &lists);
        assert_eq!(report.unplaced, vec![]);
        assert_eq!(report.audio.len(), 1);
        let role = &report.audio[0];
        assert_eq!(role.stream, 4);
        assert_eq!(role.choices, vec!["c010".to_string(), "c011".to_string()]);
        assert_eq!(role.role, Some(AudioRole::Commentary));
        assert_eq!((role.tie, role.why.as_str()), (Tie::Default, "tie default: attr: 0 languages named"));
        assert_eq!(role.confirms(), Option::None);
    }

    /// A twin's audio 2 is the block's audio 3 by PID, so its role lands on the scan stream of block audio 3.
    #[test]
    fn a_twin_playlists_choice_is_translated_by_pid() {
        let twin_audio = [(0x1100, 0x83, "eng"), (0x1102, 0x81, "eng")];
        let lists = playlists(vec![
            ("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO))),
            ("00801", Ok(playlist(&[("00001", 0, 900)], &twin_audio))),
        ]);
        let choice = audio("c010", "Commentary by Person A", "00801", 2, Level::Replayed);
        let report = roles(&map(Some(effect("00801", Some(1))), vec![choice]), &title(), &lists);
        assert_eq!(report.unplaced, vec![]);
        assert_eq!(report.audio.len(), 1);
        let role = &report.audio[0];
        assert_eq!((role.stream, role.tie, role.why.as_str()), (4, Tie::Replay, ""));
        assert_eq!(role.confirms(), Some(AudioRole::Commentary));
    }

    /// A twin entry whose PID matches but whose language or coding differs is unmatched; a bibliographic code and
    /// another case still match.
    #[test]
    fn a_twin_entry_that_differs_in_language_or_codec_is_unmatched() {
        for twin_entry in [(0x1102, 0x81, "fra"), (0x1102, 0x82, "eng")] {
            let lists = playlists(vec![
                ("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO))),
                ("00801", Ok(playlist(&[("00001", 0, 900)], &[(0x1100, 0x83, "eng"), twin_entry]))),
            ]);
            let choice = audio("c010", "Commentary by Person A", "00801", 2, Level::Replayed);
            let report = roles(&map(Some(effect("00801", Some(1))), vec![choice]), &title(), &lists);
            assert_eq!(report.audio, vec![], "{twin_entry:?}");
            let why = "unmatched: attributes differ in 00800 (menu 00801 audio 2)";
            assert_eq!(report.unplaced, vec![unplaced("c010", why)], "{twin_entry:?}");
        }
        let lists = playlists(vec![
            ("00800", Ok(playlist(&[("00001", 0, 900)], &[(0x1100, 0x83, "ENG"), (0x1102, 0x81, "ger")]))),
            ("00801", Ok(playlist(&[("00001", 0, 900)], &[(0x1102, 0x81, "deu")]))),
        ]);
        let scan = scan_title("00800.mpls", &[("eng", "TrueHD", "0"), ("deu", "AC3", "0")]);
        let choice = audio("c010", "Commentary by Person A", "00801", 1, Level::Replayed);
        let report = roles(&map(play(), vec![choice]), &scan, &lists);
        assert_eq!(report.unplaced, vec![]);
        assert_eq!((report.audio[0].stream, report.audio[0].tie), (2, Tie::Replay));
    }

    /// A twin's PID that the block lacks or holds twice, or a number the twin lacks, is unmatched.
    #[test]
    fn a_pid_the_block_lacks_is_unmatched() {
        let twin = |audio: &[(u16, u8, &'static str)]| playlist(&[("00001", 0, 900)], audio);
        let doubled = [(0x1100, 0x83, "eng"), (0x1102, 0x81, "eng"), (0x1102, 0x81, "eng")];
        let cases = [
            (twin(&[(0x1100, 0x83, "eng"), (0x1105, 0x81, "eng")]), BLOCK_AUDIO.to_vec(), 2, "pid not in 00800"),
            (twin(&[(0x1100, 0x83, "eng"), (0x1102, 0x81, "eng")]), doubled.to_vec(), 2, "pid twice in 00800"),
        ];
        for (menu, block_audio, number, why) in cases {
            let lists = playlists(vec![
                ("00800", Ok(playlist(&[("00001", 0, 900)], &block_audio))),
                ("00801", Ok(menu)),
            ]);
            let choice = audio("c010", "Commentary by Person A", "00801", number, Level::Replayed);
            let report = roles(&map(play(), vec![choice]), &title(), &lists);
            assert_eq!(report.audio, vec![]);
            assert_eq!(report.unplaced, vec![unplaced("c010", &format!("unmatched: {why} (menu 00801 audio 2)"))]);
        }
        let lists = playlists(vec![
            ("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO))),
            ("00801", Ok(playlist(&[("00001", 0, 900)], &[(0x1100, 0x83, "eng")]))),
        ]);
        for number in [0, 2, -1, i64::MAX, i64::MIN] {
            let choice = audio("c010", "Commentary by Person A", "00801", number, Level::Replayed);
            let report = roles(&map(play(), vec![choice]), &title(), &lists);
            let why = format!("unmatched: no audio {number} in the menu playlist");
            assert_eq!(report.unplaced, vec![unplaced("c010", &why)]);
        }
    }

    /// Under 9/10 shared (a block item counted once, a playlist of no ticks), or a menu playlist that is not uniform
    /// or not read, is another playlist; exactly 9/10 is a twin.
    #[test]
    fn a_playlist_sharing_under_nine_tenths_is_another_playlist() {
        let cases = [
            (vec![("00001", 0, 900), ("00009", 0, 101)], false),
            (vec![("00001", 0, 900), ("00009", 0, 100)], true),
            (vec![("00001", 0, 900), ("00001", 0, 900)], false),
            (vec![("00001", 0, 0)], false),
        ];
        for (items, twin) in cases {
            let lists = playlists(vec![
                ("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO))),
                ("00900", Ok(playlist(&items, &BLOCK_AUDIO))),
            ]);
            let choice = audio("c010", "Commentary by Person A", "00900", 3, Level::Replayed);
            let report = roles(&map(play(), vec![choice]), &title(), &lists);
            if twin {
                assert_eq!((report.audio.len(), report.unplaced.len()), (1, 0), "{items:?}");
            } else {
                assert_eq!(report.audio, vec![], "{items:?}");
                assert_eq!(report.unplaced, vec![unplaced("c010", "other playlist 00900")], "{items:?}");
            }
        }
        let mut uneven = playlist(&[("00001", 0, 900)], &BLOCK_AUDIO);
        uneven.uniform = false;
        for menu in [Ok(uneven), Err("the playlist has no play item".to_string())] {
            let lists = playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO))), ("00900", menu)]);
            let choice = audio("c010", "Commentary by Person A", "00900", 3, Level::Replayed);
            let report = roles(&map(play(), vec![choice]), &title(), &lists);
            assert_eq!(report.unplaced, vec![unplaced("c010", "other playlist 00900")]);
        }
    }

    /// A replayed commentary on a playlist that shares nothing with the block, or was not read, labels no stream; an
    /// odd playlist name is quoted.
    #[test]
    fn a_choice_on_another_playlist_never_labels_the_titles_streams() {
        let unrelated = playlists(vec![
            ("00800", Ok(playlist(&[("00001", 0, 900)], &BLOCK_AUDIO))),
            ("00900", Ok(playlist(&[("00002", 0, 900)], &BLOCK_AUDIO))),
        ]);
        for lists in [unrelated, block_only()] {
            let choice = audio("c010", "Commentary by Person A", "00900", 3, Level::Replayed);
            let report = roles(&map(play(), vec![choice]), &title(), &lists);
            assert_eq!(report.untied, Option::None);
            assert_eq!(report.audio, vec![]);
            assert_eq!(report.unplaced, vec![unplaced("c010", "other playlist 00900")]);
        }
        let odd = audio("c010", "Commentary by Person A", "00\n900", 3, Level::Replayed);
        let report = roles(&map(play(), vec![odd]), &title(), &block_only());
        assert_eq!(report.unplaced, vec![unplaced("c010", "other playlist '00\\n900'")]);
    }

    /// Every reason an audio choice names no stream, numbers out of range of any size included; other kinds are passed
    /// over.
    #[test]
    fn choices_that_name_no_stream_are_unplaced_with_their_reason() {
        let four = [&BLOCK_AUDIO[..], &[(0x1103, 0x81, "eng")]].concat();
        let lists = playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &four)))]);
        let choices = vec![
            choice("c001", Kind::Audio, "AUDIO", "Commentary", Option::None, Level::Read),
            choice("c002", Kind::Audio, "AUDIO", "Commentary", Some(effect("00800", Option::None)), Level::Replayed),
            audio("c003", "Commentary", "00800", 0, Level::Replayed),
            audio("c004", "Commentary", "00800", 5, Level::Replayed),
            audio("c005", "Commentary", "00800", i64::MAX, Level::Replayed),
            audio("c006", "Commentary", "00800", -3, Level::Replayed),
            audio("c007", "Commentary", "00800", 4, Level::Replayed),
            choice("c008", Kind::Subtitle, "SUBTITLES", "Commentary", Some(effect("00800", Some(2))), Level::Replayed),
            choice("c009", Kind::Other, "EXTRAS", "Commentary", Some(effect("00800", Some(2))), Level::Replayed),
        ];
        let report = roles(&map(play(), choices), &title(), &lists);
        assert_eq!(report.audio, vec![]);
        assert_eq!(
            report.unplaced,
            vec![
                unplaced("c001", "no effect"),
                unplaced("c002", "no stream number"),
                unplaced("c003", "unmatched: no audio 0 in 00800"),
                unplaced("c004", "unmatched: no audio 5 in 00800"),
                unplaced("c005", &format!("unmatched: no audio {} in 00800", i64::MAX)),
                unplaced("c006", "unmatched: no audio -3 in 00800"),
                unplaced("c007", "no scan stream for audio 4"),
            ]
        );
    }

    /// An entry whose language or codec differs from its scan stream, or a block that lists another number of audio
    /// streams than the scan, is a tie conflict and confirms nothing (MR-9).
    #[test]
    fn a_wrong_order_tie_is_a_tie_conflict() {
        let one_entry_differs = [
            ([(0x1100, 0x83, "eng"), (0x1101, 0x81, "eng"), (0x1102, 0x81, "fra")], "language fra vs eng"),
            ([(0x1100, 0x83, "eng"), (0x1101, 0x81, "eng"), (0x1102, 0x83, "eng")], "codec 0x83 vs Dolby Digital"),
        ];
        for (block_audio, why) in one_entry_differs {
            let lists = playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &block_audio)))]);
            let choices =
                vec![audio("c009", "Commentary by Person B", "00800", 2, Level::Replayed), commentary(Level::Replayed)];
            let report = roles(&map(play(), choices), &title(), &lists);
            let ties: Vec<(u32, Tie, &str)> =
                report.audio.iter().map(|role| (role.stream, role.tie, role.why.as_str())).collect();
            assert_eq!(ties, vec![(3, Tie::Replay, ""), (4, Tie::TieConflict, why)]);
            assert_eq!(report.audio[1].role, Some(AudioRole::Commentary));
            assert_eq!(report.audio[1].confirms(), Option::None);
        }
        let other_counts = [
            (BLOCK_AUDIO[..2].to_vec(), "the playlist lists 2 audio streams, the scan 3"),
            ([&BLOCK_AUDIO[..], &[(0x1103, 0x81, "eng")]].concat(), "the playlist lists 4 audio streams, the scan 3"),
        ];
        for (block_audio, why) in other_counts {
            let lists = playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &block_audio)))]);
            let choice = audio("c009", "Commentary by Person B", "00800", 2, Level::Replayed);
            let report = roles(&map(play(), vec![choice]), &title(), &lists);
            let role = &report.audio[0];
            assert_eq!((role.stream, role.tie, role.why.as_str()), (3, Tie::TieConflict, why));
            assert_eq!(role.confirms(), Option::None);
        }
    }

    /// DTS-HD MA under 0x86, a language `und`, a bibliographic code and a coding outside the table make no conflict;
    /// DTS-HD MA under 0x85 does.
    #[test]
    fn a_codec_family_the_coding_allows_and_an_unknown_language_make_no_conflict() {
        let block_audio = [(0x1100, 0x86, "eng"), (0x1101, 0x86, "und"), (0x1102, 0xA1, "fre")];
        let lists = playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &block_audio)))]);
        let scan_audio = [("eng", "DTS-HD MA", "0"), ("fra", "DTS-HD MA", "0"), ("fra", "AC3", "0")];
        let scan = scan_title("00800.mpls", &scan_audio);
        let choices = vec![
            audio("c010", "Commentary by Person A", "00800", 2, Level::Replayed),
            audio("c011", "Commentary by Person B", "00800", 3, Level::Replayed),
        ];
        let report = roles(&map(play(), choices), &scan, &lists);
        let ties: Vec<(u32, Tie)> = report.audio.iter().map(|role| (role.stream, role.tie)).collect();
        assert_eq!(ties, vec![(2, Tie::Replay), (3, Tie::Replay)]);
        let block_audio = [(0x1100, 0x86, "eng"), (0x1101, 0x85, "eng")];
        let lists = playlists(vec![("00800", Ok(playlist(&[("00001", 0, 900)], &block_audio)))]);
        let scan = scan_title("00800.mpls", &[("eng", "DTS-HD MA", "0"), ("eng", "DTS-HD MA", "0")]);
        let choice = audio("c010", "Commentary by Person A", "00800", 2, Level::Replayed);
        let report = roles(&map(play(), vec![choice]), &scan, &lists);
        assert_eq!(
            (report.audio[0].tie, report.audio[0].why.as_str()),
            (Tie::TieConflict, "codec 0x85 vs DTS-HD MA")
        );
    }

    /// Without the block, read and uniform, the title is untied with nothing placed; unused decisions are still listed
    /// (MR-8).
    #[test]
    fn without_the_titles_playlist_no_stream_is_tied() {
        let mut bare = title();
        bare.source_file = Option::None;
        let mut uneven = playlist(&[("00001", 0, 900)], &BLOCK_AUDIO);
        uneven.uniform = false;
        let cases = [
            (bare, block_only(), "the title has no playlist"),
            (title(), playlists(vec![]), "playlist 00800 was not read"),
            (
                title(),
                playlists(vec![("00800", Err("the playlist is cut short at byte 0".to_string()))]),
                "playlist 00800: the playlist is cut short at byte 0",
            ),
            (
                title(),
                playlists(vec![("00800", Ok(uneven))]),
                "playlist 00800: its play items carry differing stream tables",
            ),
        ];
        let off = parse_off("off\tc099\n").unwrap();
        for (title, lists, why) in cases {
            let no_effect = choice("c001", Kind::Audio, "AUDIO", "X", Option::None, Level::Read);
            let choices = vec![commentary(Level::Replayed), no_effect];
            let report = audio_roles(&map(play(), choices), &off, &title, &lists);
            assert_eq!(report.untied.as_deref(), Some(why));
            assert_eq!((report.audio, report.unplaced), (vec![], vec![]), "{why}");
            assert_eq!(report.off_unused, vec!["c099".to_string()], "{why}");
        }
    }

    /// "COMMENTARY: Off" without a decision gives no role; `off` leaves no role word, `role` reads it (MR-6). Two roles
    /// are several.
    #[test]
    fn an_undecided_off_text_gives_no_role_and_a_decision_settles_it() {
        let switch = choice("c010", Kind::Audio, "COMMENTARY", "Off", Some(effect("00800", Some(3))), Level::Replayed);
        let cases = [
            ("", Option::None, "undecided: c010 (menu-off.tsv)"),
            ("off\tc010\n", Option::None, "no role word"),
            ("role\tc010\n", Some(AudioRole::Commentary), ""),
        ];
        for (text, role, why) in cases {
            let off = parse_off(text).unwrap();
            let report = audio_roles(&map(play(), vec![switch.clone()]), &off, &title(), &block_only());
            let stream = &report.audio[0];
            assert_eq!((stream.role, stream.tie, stream.why.as_str()), (role, Tie::Replay, why), "{text:?}");
            assert_eq!(stream.confirms(), role, "{text:?}");
            assert_eq!(report.off_unused, Vec::<String>::new());
        }
        let both = vec![commentary(Level::Replayed), audio("c011", "Main Feature", "00800", 3, Level::Replayed)];
        let report = roles(&map(play(), both), &title(), &block_only());
        assert_eq!(report.audio[0].why, "several roles: commentary, programme");
        assert_eq!(report.audio[0].confirms(), Option::None);
    }

    /// A programme role with a strong tie confirms nothing; an audio description does.
    #[test]
    fn a_programme_role_confirms_nothing() {
        let choice = audio("c010", "Main Feature", "00800", 3, Level::Replayed);
        let report = roles(&map(play(), vec![choice]), &title(), &block_only());
        let role = &report.audio[0];
        assert_eq!((role.role, role.tie, role.why.as_str()), (Some(AudioRole::Programme), Tie::Replay, ""));
        assert_eq!(role.confirms(), Option::None);
        let description = audio("c011", "Audio Description", "00800", 3, Level::Replayed);
        let report = roles(&map(play(), vec![description]), &title(), &block_only());
        assert_eq!(report.audio[0].confirms(), Some(AudioRole::AudioDescription));
    }

    /// A decision on an unknown id, a subtitle choice and an audio choice without a role word is unused.
    #[test]
    fn unused_decisions_are_listed() {
        let choices = vec![
            commentary(Level::Replayed),
            choice("c020", Kind::Subtitle, "SUBTITLES", "Commentary", Some(effect("00800", Some(1))), Level::Replayed),
            audio("c021", "Language A", "00800", 2, Level::Replayed),
        ];
        let off = parse_off("off\tc099\noff\tc020\nrole\tc021\nrole\tc010\n").unwrap();
        let report = audio_roles(&map(play(), choices), &off, &title(), &block_only());
        assert_eq!(report.off_unused, vec!["c020".to_string(), "c021".to_string(), "c099".to_string()]);
    }

    /// The block acts on a matching identity only, and never on an untied report (MR-5, MR-8).
    #[test]
    fn the_block_acts_only_on_a_matching_identity() {
        let report = roles(&map(play(), vec![commentary(Level::Replayed)]), &title(), &block_only());
        let block = report.clone().block(&Identity::Matches("0123".to_string()));
        assert_eq!((block.acts, block.why.as_str()), (true, ""));
        assert_eq!((block.audio, block.unplaced, block.off_unused), (report.audio.clone(), vec![], vec![]));
        let block = report.clone().block(&Identity::NotChecked("the map has no disc_id_sha1".to_string()));
        assert_eq!(
            (block.acts, block.why.as_str()),
            (false, "the map's identity is not checked: the map has no disc_id_sha1")
        );
        let differs = Identity::Differs { map: "0123".to_string(), disc: "4567".to_string() };
        let block = report.block(&differs);
        assert_eq!(
            (block.acts, block.why.as_str()),
            (false, "the map's identity is disc_id differs: the map's is 0123, the disc's is 4567")
        );
        let untied = roles(&map(play(), vec![commentary(Level::Replayed)]), &title(), &playlists(vec![]));
        let block = untied.block(&Identity::Matches("0123".to_string()));
        assert_eq!((block.acts, block.why.as_str()), (false, "playlist 00800 was not read"));
        let text = serde_json::to_string(&block).unwrap();
        let want = r#"{"acts":false,"why":"playlist 00800 was not read","audio":[],"unplaced":[],"off_unused":[]}"#;
        assert_eq!(text, want);
    }

    /// `title_block` gives no roles, and the printed block has no `roles` key, so it prints as before.
    #[test]
    fn a_title_block_carries_no_roles_and_prints_no_roles_key() {
        let disc = crate::disc::disc(&crate::robot::parse(&crate::testsupport::menu_scan_text())).unwrap();
        let identity = Identity::NotChecked("the map has no disc_id_sha1".to_string());
        let block = title_block(&map(play(), vec![commentary(Level::Replayed)]), &disc, &disc.titles[&0], &identity);
        assert_eq!(block.roles, Option::None);
        assert!(!serde_json::to_string(&block).unwrap().contains("roles"));
    }
}
