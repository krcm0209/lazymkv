//! Naming (design spec 8.9): the resolution label, the movie path under `movies_root` in the three layouts of
//! `edition_style`, the TV path under `tv_root` with multi-episode files and specials (`Season 00`), the second ID
//! tag of `jellyfin10_compat` with its note `compat_tag_omitted`, and the sanitisation of every path component; the
//! 32-character cap of edition names (spec 8.4) and the 90-character cap of episode titles (8.9); the distinguishing
//! label spec 11's "Resolve collision" gives the new file when the owner keeps both (`- 2160p 2`). Pure (RP19): a
//! root is the settings' text and a path takes its root's own separator, never the host's (`join`). Written from the
//! spec (RP20). Task 13's `decision::decide` builds the final path with it; Task 16 prints it.

use unicode_normalization::UnicodeNormalization;

use crate::lookup::tmdb::{TmdbKind, Work};
use crate::review::{Note, Noted};
use crate::settings::{EditionStyle, Settings};

/// The resolution label of spec 8.9, from the height of the first video track (`disc::Title::video_height`, the H of
/// attribute 19): 2000 or more is "2160p", 1000 or more "1080p", 700 or more "720p", exactly 576 "576p", anything
/// else "480p". Every movie file carries it (D11), so the 1080p disc of a combo pack files beside the 2160p copy.
pub fn resolution_label(height: u32) -> &'static str {
    if height >= 2000 {
        "2160p"
    } else if height >= 1000 {
        "1080p"
    } else if height >= 700 {
        "720p"
    } else if height == 576 {
        "576p"
    } else {
        "480p"
    }
}

/// The device names Windows reserves (spec 8.9 "a reserved device name gets `_` appended"), as Microsoft's "Naming
/// Files, Paths, and Namespaces" lists them: CON, PRN, AUX, NUL, COM0 to COM9, COM with a superscript 1, 2 or 3
/// (U+00B9, U+00B2, U+00B3), LPT0 to LPT9 and LPT with a superscript 1, 2 or 3. Compared ignoring ASCII case only, so
/// the superscript digits must match exactly.
pub const RESERVED_NAMES: [&str; 30] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "COM\u{b9}", "COM\u{b2}", "COM\u{b3}", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    "LPT\u{b9}", "LPT\u{b2}", "LPT\u{b3}",
];

/// Spec 8.9 "each component capped at 255 bytes": UTF-8 bytes, cut on a character boundary.
pub const COMPONENT_MAX_BYTES: usize = 255;

/// Spec 8.4: edition names are "capped at 32 characters" (Unicode scalar values, counted after NFC).
pub const EDITION_MAX_CHARS: usize = 32;

/// Spec 8.9 "Episode titles are cut to 90 characters" (Unicode scalar values, counted after NFC).
pub const EPISODE_TITLE_MAX_CHARS: usize = 90;

/// The characters spec 8.9 removes from every component.
const FORBIDDEN: [char; 8] = ['/', '\\', '|', '?', '*', '"', '<', '>'];

/// The characters spec 8.9 removes from titles and edition names.
const BRACKETS: [char; 4] = ['[', ']', '{', '}'];

/// Spec 8.9's sanitisation of one path component, in this order: Unicode NFC; `:` replaced by " -"; `/ \ | ? * " < >`
/// and control characters removed; with `strip_brackets` (a title or an edition name), `[ ] { }` removed; runs of
/// spaces collapsed to one; leading spaces and periods dropped (a name that begins with `.` is hidden on the NAS and
/// skipped by the servers' scans; the owner's confirmed addition to 8.9's list (ruling OR4)); trailing spaces and
/// periods trimmed; a reserved device name (`RESERVED_NAMES`, judged on the stem before the first `.`, ignoring ASCII
/// case and the stem's trailing spaces) gets `_` appended to its stem (`con.mkv` gives `con_.mkv`); capped at
/// COMPONENT_MAX_BYTES on a character boundary, and trimmed again of the trailing spaces and periods the cap may
/// leave.
pub fn sanitise(component: &str, strip_brackets: bool) -> String {
    sanitise_within(component, strip_brackets, COMPONENT_MAX_BYTES)
}

/// An edition name as spec 8.4 files it: "sanitised like titles and capped at 32 characters". The character rules of
/// `sanitise` with brackets removed, the first EDITION_MAX_CHARS characters, trailing spaces and periods trimmed. The
/// reserved-name rule is left to the component the name ends up in (an edition `Aux` stays `Aux`). `movie_path` takes
/// its edition through this function.
pub fn edition_label(name: &str) -> String {
    capped_text(name, EDITION_MAX_CHARS)
}

/// An episode title as spec 8.9 files it: the character rules of `sanitise` with brackets removed (a slash in
/// `Part One/Part Two` is removed), the first EPISODE_TITLE_MAX_CHARS characters, trailing spaces and
/// periods trimmed. `tv_path` takes its episode name through this function.
pub fn episode_title(name: &str) -> String {
    capped_text(name, EPISODE_TITLE_MAX_CHARS)
}

/// `sanitise` with a byte cap of `max_bytes`: the file name's stem is capped at 251 bytes so ".mkv" fits in 255.
fn sanitise_within(component: &str, strip_brackets: bool, max_bytes: usize) -> String {
    let cleaned = clean(component, strip_brackets);
    let visible = cleaned.trim_start_matches(|ch: char| ch == '.' || ch == ' ');
    let named = with_reserved_suffix(trim_end_spaces_and_periods(visible));
    trim_end_spaces_and_periods(cap_bytes(&named, max_bytes)).to_string()
}

/// `clean` with brackets removed, cut to `max_chars` characters, trailing spaces and periods trimmed before and after the cut.
fn capped_text(text: &str, max_chars: usize) -> String {
    let cleaned = clean(text, true);
    let cut = cap_chars(trim_end_spaces_and_periods(&cleaned), max_chars);
    trim_end_spaces_and_periods(cut).to_string()
}

/// The character rules of spec 8.9 on a piece of text: NFC, `:` to " -", the forbidden and control characters
/// removed, brackets removed when `strip_brackets`, runs of spaces collapsed, leading and trailing spaces dropped.
/// Periods are kept: only a component's end is trimmed of them.
fn clean(text: &str, strip_brackets: bool) -> String {
    let normalised: String = text.nfc().collect();
    let mut kept = String::with_capacity(normalised.len() + 8);
    for ch in normalised.chars() {
        if ch == ':' {
            kept.push_str(" -");
        } else if !(FORBIDDEN.contains(&ch) || ch.is_control() || (strip_brackets && BRACKETS.contains(&ch))) {
            kept.push(ch);
        }
    }
    collapse_spaces(&kept)
}

/// The words of `text` between spaces, joined by one space: runs of spaces collapsed, leading and trailing ones dropped.
fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split(' ').filter(|word| !word.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// `text` without its trailing spaces and periods.
fn trim_end_spaces_and_periods(text: &str) -> &str {
    text.trim_end_matches(|ch: char| ch == ' ' || ch == '.')
}

/// The longest prefix of `text` of at most `max_bytes` bytes that ends on a character boundary.
fn cap_bytes(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// The first `max_chars` characters of `text`.
fn cap_chars(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

/// `component` with `_` appended to its stem (the text before the first `.`, without trailing spaces) when the stem is
/// a reserved device name; otherwise unchanged.
fn with_reserved_suffix(component: &str) -> String {
    let (stem, rest) = match component.find('.') {
        Some(dot) => component.split_at(dot),
        None => (component, ""),
    };
    let stem = stem.trim_end_matches(' ');
    if RESERVED_NAMES.iter().any(|name| name.eq_ignore_ascii_case(stem)) {
        format!("{stem}_{rest}")
    } else {
        component.to_string()
    }
}

/// Spec 8.9: "Jellyfin 10.x reads a number between 1900 and 2099 as a year", so such a TMDb ID cannot be its tag.
pub fn year_like(id: i64) -> bool {
    (1900..=2099).contains(&id)
}

/// Spec 8.9's second ID tag, with its leading space, when settings.jellyfin10_compat is on: " [tmdbid-<TMDb ID>]";
/// for a year-like TMDb ID, " [imdbid-<IMDb ID>]" for a movie (the IMDb ID as TMDb gives it, "tt0004242") or
/// " [tvdbid-<TVDB ID>]" for a show. None in exactly two cases: compat is off, or the ID is year-like and the IMDb
/// (movie) or TVDB (show) ID is absent or empty; only the second makes `movie_path` and `tv_path` add the note
/// compat_tag_omitted. The year plays no part. Reads settings.jellyfin10_compat.
pub fn compat_tag(work: &Work, settings: &Settings) -> Option<String> {
    if !settings.jellyfin10_compat {
        return None;
    }
    if !year_like(work.tmdb_id) {
        return Some(format!(" [tmdbid-{}]", work.tmdb_id));
    }
    match work.kind {
        TmdbKind::Movie => work.imdb_id.as_deref().filter(|id| !id.is_empty()).map(|id| format!(" [imdbid-{id}]")),
        TmdbKind::Tv => work.tvdb_id.map(|id| format!(" [tvdbid-{id}]")),
    }
}

/// The final file's extension; the 255-byte cap of a file name keeps it.
const EXTENSION: &str = ".mkv";

/// A final path (spec 8.9) as sanitised components under a library root, and the components joined under the root
/// with the root's own separator (`join`). Task 13 puts it in the `FileDecision`; phase 2 files to `path`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NamedPath {
    /// The library root as the settings give it (`movies_root` or `tv_root`), unchanged.
    pub root: String,
    /// The components under the root, each through `sanitise`, in order: the work's folder (and for TV the season
    /// folder), then the file name.
    pub components: Vec<String>,
    /// `join(root, components)`.
    pub path: String,
}

/// The components joined under `root`: with '\\' when the root holds a backslash (a drive path `D:\Media` or a UNC
/// path `\\server\share`, which starts with two), else with '/'; no separator is added after a root that already ends
/// in one, and under an empty root the components are joined with '/' without a leading separator. `core` never asks
/// the host which separator it uses (spec 8.9's paths are the share's).
pub fn join(root: &str, components: &[String]) -> String {
    let separator = if root.contains('\\') { '\\' } else { '/' };
    let mut path = root.to_string();
    for component in components {
        if !path.is_empty() && !path.ends_with('/') && !path.ends_with('\\') {
            path.push(separator);
        }
        path.push_str(component);
    }
    path
}

/// Spec 8.9's movie path under `root` (settings.movies_root): the folder `Title (Year) {tmdb-ID}[ compat]` and the
/// file `Title (Year) {tmdb-ID}[ compat] - <Res>[ <n>]`, laid out by settings.edition_style when `edition` is given:
/// Combined appends ` <Ed> {edition-<Ed>}` to the file name; Plex appends ` {edition-<Ed>}` to the folder and the file
/// name before ` - <Res>`; Jellyfin appends ` <Ed>` to the file name. `work` is the TMDb record's Work (title, year and
/// ID from one record); `resolution` is `resolution_label`'s; `edition` is already through `edition_label` (8.4), and
/// None, empty or blank means no edition; a year of None writes no " (Year)"; `copy` Some(n) appends " n" after the
/// resolution label (`- 2160p 2`, spec 11 "Resolve collision", keep both). The compat tag is `compat_tag`'s, in both
/// components; when compat is on and the tag cannot be built, the note compat_tag_omitted is returned. The title goes
/// through the character rules with brackets removed and is cut by `title_within` so that the file name, the longer
/// component, fits in 255 bytes with everything after the title: the file name always begins with the folder name and
/// keeps its tags and labels. Each component then goes through `sanitise`. Reads settings.edition_style and
/// settings.jellyfin10_compat.
pub fn movie_path(
    root: &str,
    work: &Work,
    resolution: &str,
    edition: Option<&str>,
    copy: Option<u32>,
    settings: &Settings,
) -> (NamedPath, Vec<Noted>) {
    let mut notes = Vec::new();
    let tag = compat_or_note(work, settings, &mut notes);
    // ` (Year) {tmdb-ID}[ compat]`: what follows the title in both components.
    let tagged = format!("{} {{tmdb-{}}}{tag}", year_suffix(work), work.tmdb_id);
    let label = match copy {
        Some(n) => format!("{resolution} {n}"),
        None => resolution.to_string(),
    };
    let edition = edition.map(str::trim).filter(|name| !name.is_empty());
    // What follows the title in the folder and in the file name's stem.
    let (folder, stem) = match (edition, settings.edition_style) {
        (None, _) => (tagged.clone(), format!("{tagged} - {label}")),
        (Some(name), EditionStyle::Combined) => (tagged.clone(), format!("{tagged} - {label} {name} {{edition-{name}}}")),
        (Some(name), EditionStyle::Plex) => {
            let folder = format!("{tagged} {{edition-{name}}}");
            let stem = format!("{folder} - {label}");
            (folder, stem)
        }
        (Some(name), EditionStyle::Jellyfin) => (tagged.clone(), format!("{tagged} - {label} {name}")),
    };
    let title = title_within(&work.title, stem.len() + EXTENSION.len());
    let components = vec![sanitise(&format!("{title}{folder}"), false), file_name(&format!("{title}{stem}"))];
    (named(root, components), notes)
}

/// `compat_tag`'s tag, or "" with the note compat_tag_omitted pushed when compat is on and the tag cannot be built
/// (a year-like TMDb ID without an IMDb ID for a movie or a TVDB ID for a show; spec 8.9).
fn compat_or_note(work: &Work, settings: &Settings, notes: &mut Vec<Noted>) -> String {
    if let Some(tag) = compat_tag(work, settings) {
        return tag;
    }
    if settings.jellyfin10_compat {
        let other = match work.kind {
            TmdbKind::Movie => "IMDb",
            TmdbKind::Tv => "TVDB",
        };
        notes.push(Noted {
            note: Note::CompatTagOmitted,
            stream: None,
            detail: format!(
                "TMDb ID {} reads as a year to Jellyfin 10.x and TMDb gives no {other} ID: the compatibility tag is left out",
                work.tmdb_id
            ),
        });
    }
    String::new()
}

/// " (Year)", or "" for a work without a year: what follows the title in `Title (Year)` (spec 8.9).
fn year_suffix(work: &Work) -> String {
    match work.year {
        Some(year) => format!(" ({year})"),
        None => String::new(),
    }
}

/// The work's title through the character rules with brackets removed (its periods kept; only a component's end is
/// trimmed), cut on a character boundary so that `suffix_bytes` more bytes still fit in COMPONENT_MAX_BYTES; a title the
/// cut shortens is trimmed of the trailing spaces and periods the cut leaves. The caller passes the bytes that follow the
/// title in its longest component, so spec 8.9's 255-byte cap takes bytes from the title and never from the ID tags, the
/// labels or the episode code after it.
fn title_within(title: &str, suffix_bytes: usize) -> String {
    let cleaned = clean(title, true);
    let budget = COMPONENT_MAX_BYTES.saturating_sub(suffix_bytes);
    if cleaned.len() <= budget {
        return cleaned;
    }
    trim_end_spaces_and_periods(cap_bytes(&cleaned, budget)).to_string()
}

/// A file name: the stem sanitised within 255 bytes less the extension's 4, then ".mkv".
fn file_name(stem: &str) -> String {
    format!("{}{EXTENSION}", sanitise_within(stem, false, COMPONENT_MAX_BYTES - EXTENSION.len()))
}

/// The NamedPath of `components` under `root`.
fn named(root: &str, components: Vec<String>) -> NamedPath {
    let path = join(root, &components);
    NamedPath { root: root.to_string(), components, path }
}

/// Spec 8.9's TV path under `root` (settings.tv_root): the show folder `Show (Year) {tmdb-ID}[ compat]`, the season
/// folder `Season NN` (season 0 is `Season 00`, the specials), and the file `Show (Year) - SNNENN[-ENN][ <n>][ - Episode
/// Title]`. One episode writes `S03E16`; two or more write the lowest and the highest (`S03E12-E13`, 8.9's
/// multi-episode form); an empty list writes the season code alone. The compat tag (`compat_tag`) is on the show
/// folder only, never on the season folder or the file, with the note compat_tag_omitted when compat is on and the
/// tag cannot be built. `episode_name` goes through `episode_title`; when it is None or cleans to nothing, the title
/// and its " - " are left out. `copy` Some(n) appends " n" after the episode code (spec 11 "Resolve collision", keep
/// both). A year of None writes no " (Year)". The show's title is cut by `title_within` so that the show folder and the
/// file name up to its episode code fit in 255 bytes; the file name's own cap then takes any excess from the episode
/// title at its end. Reads settings.jellyfin10_compat.
pub fn tv_path(
    root: &str,
    work: &Work,
    season: u32,
    episodes: &[u32],
    episode_name: Option<&str>,
    copy: Option<u32>,
    settings: &Settings,
) -> (NamedPath, Vec<Noted>) {
    let mut notes = Vec::new();
    let tag = compat_or_note(work, settings, &mut notes);
    let year = year_suffix(work);
    // What follows the show's title in the show folder, and in the file name up to the episode title.
    let folder = format!("{year} {{tmdb-{}}}{tag}", work.tmdb_id);
    let mut code = format!("{year} - {}", episode_code(season, episodes));
    if let Some(n) = copy {
        code = format!("{code} {n}");
    }
    let show = title_within(&work.title, folder.len().max(code.len() + EXTENSION.len()));
    let season_folder = format!("Season {season:02}");
    let mut stem = format!("{show}{code}");
    if let Some(name) = episode_name.map(episode_title).filter(|name| !name.is_empty()) {
        stem = format!("{stem} - {name}");
    }
    let components = vec![sanitise(&format!("{show}{folder}"), false), sanitise(&season_folder, false), file_name(&stem)];
    (named(root, components), notes)
}

/// `SNNENN` for one episode, `SNNENN-ENN` from the lowest to the highest of two or more, `SNN` for none (spec 8.9).
fn episode_code(season: u32, episodes: &[u32]) -> String {
    match (episodes.iter().min(), episodes.iter().max()) {
        (Some(first), Some(last)) if first != last => format!("S{season:02}E{first:02}-E{last:02}"),
        (Some(first), _) => format!("S{season:02}E{first:02}"),
        _ => format!("S{season:02}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_label_is_2160p_from_a_height_of_2000_up() {
        assert_eq!(resolution_label(2000), "2160p");
        assert_eq!(resolution_label(2160), "2160p");
        assert_eq!(resolution_label(4320), "2160p");
    }

    #[test]
    fn resolution_label_is_1080p_from_1000_to_1999() {
        assert_eq!(resolution_label(1000), "1080p");
        assert_eq!(resolution_label(1080), "1080p");
        assert_eq!(resolution_label(1999), "1080p");
    }

    #[test]
    fn resolution_label_is_720p_from_700_to_999() {
        assert_eq!(resolution_label(700), "720p");
        assert_eq!(resolution_label(720), "720p");
        assert_eq!(resolution_label(999), "720p");
    }

    #[test]
    fn resolution_label_is_576p_only_at_576() {
        assert_eq!(resolution_label(576), "576p");
    }

    #[test]
    fn resolution_label_is_480p_for_every_other_height() {
        assert_eq!(resolution_label(699), "480p");
        assert_eq!(resolution_label(577), "480p");
        assert_eq!(resolution_label(575), "480p");
        assert_eq!(resolution_label(480), "480p");
        assert_eq!(resolution_label(0), "480p");
    }

    #[test]
    fn sanitise_normalises_to_nfc() {
        let cleaned = sanitise("Ame\u{301}lie", true);
        assert_eq!(cleaned, "Am\u{e9}lie");
        assert_eq!(cleaned.len(), 7);
        assert_eq!(sanitise("Am\u{e9}lie", true), cleaned);
    }

    #[test]
    fn a_colon_becomes_a_space_and_a_dash() {
        assert_eq!(sanitise("Mission: Impossible", true), "Mission - Impossible");
        assert_eq!(sanitise("Star Wars: Episode IV - A New Hope", true), "Star Wars - Episode IV - A New Hope");
        assert_eq!(sanitise("10:30", false), "10 -30");
        assert_eq!(sanitise("A : B", false), "A - B");
        assert_eq!(sanitise(":Prefix", false), "-Prefix");
    }

    #[test]
    fn the_forbidden_characters_are_removed() {
        assert_eq!(sanitise(r#"a/b\c|d?e*f"g<h>i"#, false), "abcdefghi");
        assert_eq!(sanitise("Face/Off", true), "FaceOff");
        assert_eq!(sanitise("What If...?", true), "What If");
        assert_eq!(sanitise("Pan's Labyrinth & Friends!", true), "Pan's Labyrinth & Friends!");
    }

    #[test]
    fn control_characters_are_removed() {
        assert_eq!(sanitise("Line\nBreak\tTab\u{0}Zero\u{7f}Delete\u{85}Next", false), "LineBreakTabZeroDeleteNext");
    }

    #[test]
    fn brackets_are_removed_only_from_titles_and_edition_names() {
        assert_eq!(sanitise("[REC] {Uncut}", true), "REC Uncut");
        assert_eq!(sanitise("[REC] {Uncut}", false), "[REC] {Uncut}");
        assert_eq!(
            sanitise("Film (2010) {tmdb-12345} [tmdbid-12345]", false),
            "Film (2010) {tmdb-12345} [tmdbid-12345]"
        );
        assert_eq!(sanitise("Film (Uncut)", true), "Film (Uncut)");
    }

    #[test]
    fn repeated_spaces_are_collapsed_and_leading_spaces_dropped() {
        assert_eq!(sanitise("  The   Big    Sleep", true), "The Big Sleep");
        assert_eq!(sanitise("A [ ] B", true), "A B");
        assert_eq!(sanitise("Tab\t Then", false), "Tab Then");
    }

    #[test]
    fn leading_periods_are_dropped_so_no_component_is_hidden() {
        assert_eq!(sanitise("...x", false), "x");
        assert_eq!(sanitise("...And Justice for All", true), "And Justice for All");
        assert_eq!(sanitise(". . . Then", false), "Then");
        assert_eq!(sanitise(".hidden.mkv", false), "hidden.mkv");
        assert_eq!(sanitise(".con", false), "con_");
        assert_eq!(sanitise("Mr. Smith...", false), "Mr. Smith");
    }

    #[test]
    fn trailing_spaces_and_periods_are_trimmed() {
        assert_eq!(sanitise("Crazy, Stupid, Love.", true), "Crazy, Stupid, Love");
        assert_eq!(sanitise("What If . . .  ", false), "What If");
        assert_eq!(sanitise("End.?", false), "End");
        assert_eq!(sanitise("...", false), "");
        assert_eq!(sanitise("Mr. Smith", false), "Mr. Smith");
    }

    #[test]
    fn a_reserved_device_name_gets_an_underscore_whatever_its_case() {
        for name in RESERVED_NAMES {
            assert_eq!(sanitise(name, false), format!("{name}_"));
            let lower = name.to_lowercase();
            assert_eq!(sanitise(&lower, false), format!("{lower}_"));
        }
        assert_eq!(sanitise("Aux", true), "Aux_");
        assert_eq!(sanitise("aux ", false), "aux_");
        assert_eq!(sanitise("con.", false), "con_");
    }

    #[test]
    fn a_reserved_name_is_judged_by_its_stem_before_any_extension() {
        assert_eq!(sanitise("con.mkv", false), "con_.mkv");
        assert_eq!(sanitise("Nul.tar.gz", false), "Nul_.tar.gz");
        assert_eq!(sanitise("AUX .mkv", false), "AUX_.mkv");
        assert_eq!(sanitise("LPT1.Extended Cut", false), "LPT1_.Extended Cut");
    }

    #[test]
    fn names_that_only_resemble_a_device_name_are_left_alone() {
        for name in ["COM10", "LPT10", "COM\u{b9}0", "CONSOLE", "Con Air", "aux-1", "PRN2", "_con", "Nullify.mkv"] {
            assert_eq!(sanitise(name, false), name);
        }
    }

    #[test]
    fn the_reserved_names_are_the_windows_device_names() {
        assert_eq!(
            RESERVED_NAMES,
            [
                "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
                "COM9", "COM\u{b9}", "COM\u{b2}", "COM\u{b3}", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
                "LPT7", "LPT8", "LPT9", "LPT\u{b9}", "LPT\u{b2}", "LPT\u{b3}",
            ]
        );
    }

    #[test]
    fn a_superscript_device_name_is_reserved_and_compared_exactly() {
        assert_eq!(sanitise("com\u{b2}", false), "com\u{b2}_");
        assert_eq!(sanitise("LPT\u{b3}.mkv", false), "LPT\u{b3}_.mkv");
        assert_eq!(sanitise("COM2", false), "COM2_");
        assert_eq!(sanitise("COM\u{2074}", false), "COM\u{2074}");
    }

    #[test]
    fn the_caps_are_the_spec_values() {
        assert_eq!(COMPONENT_MAX_BYTES, 255);
        assert_eq!(EDITION_MAX_CHARS, 32);
        assert_eq!(EPISODE_TITLE_MAX_CHARS, 90);
    }

    #[test]
    fn the_255_byte_cap_never_splits_a_character() {
        let two_byte = sanitise(&"\u{e9}".repeat(200), true);
        assert_eq!(two_byte, "\u{e9}".repeat(127));
        assert_eq!(two_byte.len(), 254);
        let three_byte = sanitise(&"\u{65e5}".repeat(100), true);
        assert_eq!(three_byte, "\u{65e5}".repeat(85));
        assert_eq!(three_byte.len(), 255);
        let mixed = sanitise(&format!("a{}", "\u{e9}".repeat(200)), true);
        assert_eq!(mixed.len(), 255);
        assert!(mixed.ends_with('\u{e9}'));
        assert_eq!(sanitise("Short title", true), "Short title");
    }

    #[test]
    fn the_255_byte_cap_counts_bytes_after_nfc_and_trims_what_it_leaves() {
        assert_eq!(sanitise(&"e\u{301}".repeat(200), true), "\u{e9}".repeat(127));
        assert_eq!(sanitise(&format!("{} b", "a".repeat(254)), true), "a".repeat(254));
    }

    #[test]
    fn a_slash_in_an_episode_title_is_removed() {
        assert_eq!(episode_title("Part One/Part Two"), "Part OnePart Two");
        assert_eq!(sanitise("Part One/Part Two", true), "Part OnePart Two");
    }

    #[test]
    fn edition_label_caps_an_edition_name_at_32_characters() {
        assert_eq!(edition_label("The Extended Director's Cut Special Edition"), "The Extended Director's Cut Spec");
        assert_eq!(edition_label("Twenty Fifth Anniversary Edition"), "Twenty Fifth Anniversary Edition");
        assert_eq!(edition_label("Unrated Cut"), "Unrated Cut");
    }

    #[test]
    fn edition_label_counts_characters_not_bytes() {
        let label = edition_label("Version longue du r\u{e9}alisateur, \u{e9}dition sp\u{e9}ciale");
        assert_eq!(label, "Version longue du r\u{e9}alisateur, \u{e9}");
        assert_eq!(label.chars().count(), 32);
        assert_eq!(label.len(), 34);
    }

    #[test]
    fn edition_label_trims_what_the_cut_leaves_at_the_end() {
        assert_eq!(edition_label("Extended Remastered Special Cut. Of The Film"), "Extended Remastered Special Cut");
        assert_eq!(edition_label("Extended Remastered Special Cut and More"), "Extended Remastered Special Cut");
    }

    #[test]
    fn edition_label_sanitises_like_a_title() {
        assert_eq!(edition_label("Director's Cut: [Remastered] {4K}"), "Director's Cut - Remastered 4K");
        assert_eq!(edition_label("  Extended   Cut. "), "Extended Cut");
        assert_eq!(edition_label("Final Cut/Uncut?"), "Final CutUncut");
        assert_eq!(edition_label("Ultime\u{301}"), "Ultim\u{e9}");
    }

    #[test]
    fn edition_label_never_appends_an_underscore_to_a_word_that_is_a_device_name() {
        assert_eq!(edition_label("Aux"), "Aux");
        assert_eq!(edition_label("con"), "con");
        assert_eq!(episode_title("Nul"), "Nul");
    }

    #[test]
    fn episode_title_caps_at_90_characters() {
        let long = "The One Where Everybody Finds Out About the Very Long Episode Title That Goes On and On Without Ever Stopping";
        let cut = "The One Where Everybody Finds Out About the Very Long Episode Title That Goes On and On Wi";
        assert_eq!(episode_title(long), cut);
        assert_eq!(cut.chars().count(), 90);
        assert_eq!(episode_title("First Day"), "First Day");
        assert_eq!(episode_title("The Neighbour's Party"), "The Neighbour's Party");
    }

    /// A movie Work as `lookup::tmdb::TmdbRecord::work` gives it.
    fn movie(id: i64, title: &str, year: Option<i32>, imdb: Option<&str>) -> Work {
        Work {
            kind: TmdbKind::Movie,
            tmdb_id: id,
            title: title.to_string(),
            year,
            imdb_id: imdb.map(str::to_string),
            tvdb_id: None,
        }
    }

    /// A show Work as `lookup::tmdb::TmdbRecord::work` gives it.
    fn show(id: i64, title: &str, year: Option<i32>, imdb: Option<&str>, tvdb: Option<i64>) -> Work {
        Work {
            kind: TmdbKind::Tv,
            tmdb_id: id,
            title: title.to_string(),
            year,
            imdb_id: imdb.map(str::to_string),
            tvdb_id: tvdb,
        }
    }

    /// A made-up film whose TMDb ID is not year-like.
    fn example_film() -> Work {
        movie(4242, "Example Film", Some(2006), Some("tt0004242"))
    }

    /// A made-up show whose TMDb ID is not year-like.
    fn example_show() -> Work {
        show(4343, "Example Show", Some(2005), Some("tt0004343"), Some(70001))
    }

    /// Spec 8.9's example film, `Film (2010) {tmdb-12345}`.
    fn film() -> Work {
        movie(12345, "Film", Some(2010), Some("tt0012345"))
    }

    /// The defaults with jellyfin10_compat on.
    fn compat_on() -> Settings {
        Settings { jellyfin10_compat: true, ..Settings::default() }
    }

    #[test]
    fn year_like_holds_from_1900_to_2099_inclusive() {
        assert!(!year_like(1899));
        assert!(year_like(1900));
        assert!(year_like(2010));
        assert!(year_like(2099));
        assert!(!year_like(2100));
        assert!(!year_like(4242));
        assert!(!year_like(4343));
        assert!(!year_like(0));
        assert!(!year_like(-2000));
    }

    #[test]
    fn compat_tag_is_none_while_jellyfin10_compat_is_off() {
        let settings = Settings::default();
        assert!(!settings.jellyfin10_compat);
        assert_eq!(compat_tag(&example_film(), &settings), None);
        assert_eq!(compat_tag(&example_show(), &settings), None);
        assert_eq!(compat_tag(&movie(1999, "Film", Some(2010), Some("tt1234567")), &settings), None);
    }

    #[test]
    fn compat_tag_is_the_tmdbid_tag_for_an_id_that_is_not_year_like() {
        assert_eq!(compat_tag(&example_film(), &compat_on()).as_deref(), Some(" [tmdbid-4242]"));
        assert_eq!(compat_tag(&example_show(), &compat_on()).as_deref(), Some(" [tmdbid-4343]"));
        assert_eq!(compat_tag(&film(), &compat_on()).as_deref(), Some(" [tmdbid-12345]"));
        assert_eq!(compat_tag(&movie(1899, "Film", None, None), &compat_on()).as_deref(), Some(" [tmdbid-1899]"));
    }

    #[test]
    fn a_year_like_movie_id_takes_the_imdbid_tag() {
        for id in [1900, 1999, 2099] {
            let work = movie(id, "Film", Some(2010), Some("tt1234567"));
            assert_eq!(compat_tag(&work, &compat_on()).as_deref(), Some(" [imdbid-tt1234567]"), "TMDb ID {id}");
        }
    }

    #[test]
    fn a_year_like_show_id_takes_the_tvdbid_tag() {
        let work = show(2010, "Show", Some(2008), Some("tt7654321"), Some(81189));
        assert_eq!(compat_tag(&work, &compat_on()).as_deref(), Some(" [tvdbid-81189]"));
    }

    #[test]
    fn a_year_like_id_without_the_other_id_has_no_tag() {
        assert_eq!(compat_tag(&movie(1999, "Film", Some(2010), None), &compat_on()), None);
        assert_eq!(compat_tag(&movie(1999, "Film", Some(2010), Some("")), &compat_on()), None);
        assert_eq!(compat_tag(&show(2010, "Show", Some(2008), Some("tt7654321"), None), &compat_on()), None);
        let movie_with_tvdb = Work { tvdb_id: Some(81189), ..movie(1999, "Film", Some(2010), None) };
        assert_eq!(compat_tag(&movie_with_tvdb, &compat_on()), None);
    }

    #[test]
    fn compat_tag_does_not_depend_on_the_year() {
        let tagged = Some(" [imdbid-tt1234567]".to_string());
        assert_eq!(compat_tag(&movie(1999, "Film", None, Some("tt1234567")), &compat_on()), tagged);
        assert_eq!(compat_tag(&movie(1999, "Film", Some(1999), Some("tt1234567")), &compat_on()), tagged);
        assert_eq!(compat_tag(&movie(4242, "Film", None, None), &compat_on()).as_deref(), Some(" [tmdbid-4242]"));
    }

    /// The owner's movie library root (a UNC path).
    const MOVIES: &str = r"\\unas-pro\media\Movies";

    /// The defaults with one edition style and jellyfin10_compat set.
    fn styled(edition_style: EditionStyle, jellyfin10_compat: bool) -> Settings {
        Settings { edition_style, jellyfin10_compat, ..Settings::default() }
    }

    /// A path's components as text slices, for comparison with an array.
    fn parts(path: &NamedPath) -> Vec<&str> {
        path.components.iter().map(String::as_str).collect()
    }

    /// Owned components from text slices.
    fn texts(components: &[&str]) -> Vec<String> {
        components.iter().map(|component| component.to_string()).collect()
    }

    #[test]
    fn join_uses_a_backslash_under_a_unc_root() {
        assert_eq!(join(MOVIES, &texts(&["A", "B.mkv"])), r"\\unas-pro\media\Movies\A\B.mkv");
    }

    #[test]
    fn join_uses_a_backslash_under_a_drive_letter_root() {
        assert_eq!(join(r"D:\Media\Movies", &texts(&["A", "B.mkv"])), r"D:\Media\Movies\A\B.mkv");
    }

    #[test]
    fn join_uses_a_slash_under_a_posix_root() {
        assert_eq!(join("/srv/media/Movies", &texts(&["A", "B.mkv"])), "/srv/media/Movies/A/B.mkv");
        assert_eq!(join("smb://unas-pro/media/Movies", &texts(&["A", "B.mkv"])), "smb://unas-pro/media/Movies/A/B.mkv");
    }

    #[test]
    fn join_adds_no_second_separator_after_a_trailing_one() {
        assert_eq!(join(r"\\unas-pro\media\Movies\", &texts(&["A", "B.mkv"])), r"\\unas-pro\media\Movies\A\B.mkv");
        assert_eq!(join("/srv/media/Movies/", &texts(&["A", "B.mkv"])), "/srv/media/Movies/A/B.mkv");
        assert_eq!(join("/", &texts(&["A", "B.mkv"])), "/A/B.mkv");
        assert_eq!(join("", &texts(&["A", "B.mkv"])), "A/B.mkv");
        assert_eq!(join(MOVIES, &[]), MOVIES);
    }

    #[test]
    fn a_movie_path_is_the_folder_and_the_file_with_the_resolution_label() {
        let (path, notes) = movie_path(MOVIES, &example_film(), "2160p", None, None, &Settings::default());
        assert_eq!(
            path,
            NamedPath {
                root: MOVIES.to_string(),
                components: texts(&["Example Film (2006) {tmdb-4242}", "Example Film (2006) {tmdb-4242} - 2160p.mkv"]),
                path: r"\\unas-pro\media\Movies\Example Film (2006) {tmdb-4242}\Example Film (2006) {tmdb-4242} - 2160p.mkv"
                    .to_string(),
            }
        );
        assert!(notes.is_empty());
        let (blu_ray, _) = movie_path(MOVIES, &example_film(), "1080p", None, None, &Settings::default());
        assert_eq!(
            parts(&blu_ray),
            ["Example Film (2006) {tmdb-4242}", "Example Film (2006) {tmdb-4242} - 1080p.mkv"]
        );
    }

    #[test]
    fn combined_style_puts_the_edition_and_its_tag_after_the_resolution() {
        let settings = styled(EditionStyle::Combined, false);
        let (path, notes) = movie_path(MOVIES, &film(), "2160p", Some("Extended Cut"), None, &settings);
        assert_eq!(
            parts(&path),
            ["Film (2010) {tmdb-12345}", "Film (2010) {tmdb-12345} - 2160p Extended Cut {edition-Extended Cut}.mkv"]
        );
        assert!(notes.is_empty());
    }

    #[test]
    fn plex_style_gives_the_edition_its_own_folder() {
        let (path, _) = movie_path(MOVIES, &film(), "2160p", Some("Extended Cut"), None, &styled(EditionStyle::Plex, false));
        assert_eq!(
            parts(&path),
            [
                "Film (2010) {tmdb-12345} {edition-Extended Cut}",
                "Film (2010) {tmdb-12345} {edition-Extended Cut} - 2160p.mkv",
            ]
        );
    }

    #[test]
    fn jellyfin_style_puts_the_bare_edition_after_the_resolution() {
        let settings = styled(EditionStyle::Jellyfin, false);
        let (path, _) = movie_path(MOVIES, &film(), "2160p", Some("Extended Cut"), None, &settings);
        assert_eq!(parts(&path), ["Film (2010) {tmdb-12345}", "Film (2010) {tmdb-12345} - 2160p Extended Cut.mkv"]);
    }

    #[test]
    fn without_an_edition_every_style_gives_the_same_path() {
        for style in EditionStyle::ALL {
            let (path, _) = movie_path(MOVIES, &film(), "1080p", None, None, &styled(style, false));
            assert_eq!(
                parts(&path),
                ["Film (2010) {tmdb-12345}", "Film (2010) {tmdb-12345} - 1080p.mkv"],
                "{}",
                style.as_str()
            );
        }
    }

    #[test]
    fn an_empty_edition_is_no_edition() {
        for style in EditionStyle::ALL {
            for edition in ["", "   "] {
                let (path, _) = movie_path(MOVIES, &film(), "1080p", Some(edition), None, &styled(style, false));
                assert_eq!(
                    parts(&path),
                    ["Film (2010) {tmdb-12345}", "Film (2010) {tmdb-12345} - 1080p.mkv"],
                    "{} with {edition:?}",
                    style.as_str()
                );
            }
        }
    }

    #[test]
    fn a_movie_without_a_year_writes_no_year() {
        let undated = movie(12345, "Film", None, Some("tt0012345"));
        let (path, _) = movie_path(MOVIES, &undated, "1080p", None, None, &Settings::default());
        assert_eq!(parts(&path), ["Film {tmdb-12345}", "Film {tmdb-12345} - 1080p.mkv"]);
        let (cut, _) = movie_path(MOVIES, &undated, "1080p", Some("Extended Cut"), None, &Settings::default());
        assert_eq!(
            parts(&cut),
            ["Film {tmdb-12345}", "Film {tmdb-12345} - 1080p Extended Cut.mkv"]
        );
    }

    #[test]
    fn the_copy_label_follows_the_resolution_label() {
        let (plain, _) = movie_path(MOVIES, &example_film(), "2160p", None, Some(2), &Settings::default());
        assert_eq!(
            parts(&plain),
            ["Example Film (2006) {tmdb-4242}", "Example Film (2006) {tmdb-4242} - 2160p 2.mkv"]
        );
        let edition = Some("Extended Cut");
        let (combined, _) = movie_path(MOVIES, &film(), "2160p", edition, Some(2), &styled(EditionStyle::Combined, false));
        assert_eq!(
            parts(&combined),
            ["Film (2010) {tmdb-12345}", "Film (2010) {tmdb-12345} - 2160p 2 Extended Cut {edition-Extended Cut}.mkv"]
        );
        let (plex, _) = movie_path(MOVIES, &film(), "2160p", edition, Some(2), &styled(EditionStyle::Plex, false));
        assert_eq!(
            parts(&plex),
            [
                "Film (2010) {tmdb-12345} {edition-Extended Cut}",
                "Film (2010) {tmdb-12345} {edition-Extended Cut} - 2160p 2.mkv",
            ]
        );
        let (jellyfin, _) = movie_path(MOVIES, &film(), "2160p", edition, Some(2), &styled(EditionStyle::Jellyfin, false));
        assert_eq!(parts(&jellyfin), ["Film (2010) {tmdb-12345}", "Film (2010) {tmdb-12345} - 2160p 2 Extended Cut.mkv"]);
    }

    #[test]
    fn the_compat_tag_follows_the_tmdb_tag_in_the_folder_and_the_file_name() {
        let settings = styled(EditionStyle::Combined, true);
        let (path, notes) = movie_path(MOVIES, &film(), "2160p", Some("Extended Cut"), None, &settings);
        assert_eq!(
            parts(&path),
            [
                "Film (2010) {tmdb-12345} [tmdbid-12345]",
                "Film (2010) {tmdb-12345} [tmdbid-12345] - 2160p Extended Cut {edition-Extended Cut}.mkv",
            ]
        );
        assert_eq!(
            path.path,
            r"\\unas-pro\media\Movies\Film (2010) {tmdb-12345} [tmdbid-12345]\Film (2010) {tmdb-12345} [tmdbid-12345] - 2160p Extended Cut {edition-Extended Cut}.mkv"
        );
        assert!(path.components[1].starts_with(&path.components[0]));
        assert!(notes.is_empty());
    }

    #[test]
    fn the_compat_tag_sits_right_after_the_tmdb_tag_in_every_style() {
        let edition = Some("Extended Cut");
        let (plex, _) = movie_path(MOVIES, &film(), "2160p", edition, None, &styled(EditionStyle::Plex, true));
        assert_eq!(
            parts(&plex),
            [
                "Film (2010) {tmdb-12345} [tmdbid-12345] {edition-Extended Cut}",
                "Film (2010) {tmdb-12345} [tmdbid-12345] {edition-Extended Cut} - 2160p.mkv",
            ]
        );
        let (jellyfin, _) = movie_path(MOVIES, &film(), "2160p", edition, None, &styled(EditionStyle::Jellyfin, true));
        assert_eq!(
            parts(&jellyfin),
            ["Film (2010) {tmdb-12345} [tmdbid-12345]", "Film (2010) {tmdb-12345} [tmdbid-12345] - 2160p Extended Cut.mkv"]
        );
    }

    #[test]
    fn a_year_like_movie_id_puts_the_imdbid_tag_in_both_components() {
        let work = movie(1999, "Film", Some(2010), Some("tt1234567"));
        let (path, notes) = movie_path(MOVIES, &work, "1080p", None, None, &styled(EditionStyle::Combined, true));
        assert_eq!(
            parts(&path),
            ["Film (2010) {tmdb-1999} [imdbid-tt1234567]", "Film (2010) {tmdb-1999} [imdbid-tt1234567] - 1080p.mkv"]
        );
        assert!(notes.is_empty());
    }

    #[test]
    fn a_year_like_movie_id_without_an_imdb_id_omits_the_tag_with_a_note() {
        let work = movie(1999, "Film", Some(2010), None);
        let (path, notes) = movie_path(MOVIES, &work, "1080p", None, None, &styled(EditionStyle::Combined, true));
        assert_eq!(parts(&path), ["Film (2010) {tmdb-1999}", "Film (2010) {tmdb-1999} - 1080p.mkv"]);
        assert_eq!(
            notes,
            vec![Noted {
                note: Note::CompatTagOmitted,
                stream: None,
                detail: "TMDb ID 1999 reads as a year to Jellyfin 10.x and TMDb gives no IMDb ID: the compatibility tag is left out"
                    .to_string(),
            }]
        );
    }

    #[test]
    fn no_note_is_raised_while_compat_is_off() {
        let work = movie(1999, "Film", Some(2010), None);
        let (path, notes) = movie_path(MOVIES, &work, "1080p", None, None, &Settings::default());
        assert_eq!(parts(&path), ["Film (2010) {tmdb-1999}", "Film (2010) {tmdb-1999} - 1080p.mkv"]);
        assert!(notes.is_empty());
    }

    #[test]
    fn a_movie_title_is_sanitised_in_both_components() {
        let settings = Settings::default();
        let mission = movie(954, "Mission: Impossible", Some(1996), Some("tt0117060"));
        let (path, _) = movie_path(MOVIES, &mission, "1080p", None, None, &settings);
        assert_eq!(
            parts(&path),
            ["Mission - Impossible (1996) {tmdb-954}", "Mission - Impossible (1996) {tmdb-954} - 1080p.mkv"]
        );
        let (path, _) = movie_path(MOVIES, &movie(754, "Face/Off", Some(1997), None), "1080p", None, None, &settings);
        assert_eq!(parts(&path), ["FaceOff (1997) {tmdb-754}", "FaceOff (1997) {tmdb-754} - 1080p.mkv"]);
        let (path, _) = movie_path(MOVIES, &movie(8329, "[REC]", Some(2007), None), "1080p", None, None, &settings);
        assert_eq!(parts(&path), ["REC (2007) {tmdb-8329}", "REC (2007) {tmdb-8329} - 1080p.mkv"]);
        let edition = edition_label("Director's Cut: [Remastered]");
        let (path, _) = movie_path(MOVIES, &film(), "2160p", Some(edition.as_str()), None, &settings);
        assert_eq!(
            parts(&path),
            [
                "Film (2010) {tmdb-12345}",
                "Film (2010) {tmdb-12345} - 2160p Director's Cut - Remastered.mkv",
            ]
        );
    }

    #[test]
    fn a_period_inside_a_component_is_kept() {
        let settings = Settings::default();
        let love = movie(50646, "Crazy, Stupid, Love.", Some(2011), None);
        let (path, _) = movie_path(MOVIES, &love, "1080p", None, None, &settings);
        assert_eq!(
            parts(&path),
            ["Crazy, Stupid, Love. (2011) {tmdb-50646}", "Crazy, Stupid, Love. (2011) {tmdb-50646} - 1080p.mkv"]
        );
        let undated = movie(50646, "Crazy, Stupid, Love.", None, None);
        let (path, _) = movie_path(MOVIES, &undated, "1080p", None, None, &settings);
        assert_eq!(parts(&path), ["Crazy, Stupid, Love. {tmdb-50646}", "Crazy, Stupid, Love. {tmdb-50646} - 1080p.mkv"]);
    }

    #[test]
    fn a_leading_period_is_dropped_so_the_folder_is_not_hidden() {
        let justice = movie(17443, "...And Justice for All", Some(1979), None);
        let (path, _) = movie_path(MOVIES, &justice, "1080p", None, None, &Settings::default());
        assert_eq!(
            parts(&path),
            ["And Justice for All (1979) {tmdb-17443}", "And Justice for All (1979) {tmdb-17443} - 1080p.mkv"]
        );
        assert!(path.components[1].starts_with(&path.components[0]));
    }

    #[test]
    fn a_long_title_is_cut_so_both_components_keep_their_tags_within_255_bytes() {
        let long = movie(1, &"A".repeat(300), Some(2000), None);
        let (path, _) = movie_path("/srv/media/Movies", &long, "1080p", None, None, &Settings::default());
        let folder = format!("{} (2000) {{tmdb-1}}", "A".repeat(227));
        assert_eq!(path.components, [folder.clone(), format!("{folder} - 1080p.mkv")]);
        assert_eq!(path.components[0].len(), 243);
        assert_eq!(path.components[1].len(), 255);
        assert!(path.components[1].starts_with(&path.components[0]));
        assert!(path.components[1].ends_with(" - 1080p.mkv"));
        assert_eq!(path.path, format!("/srv/media/Movies/{folder}/{folder} - 1080p.mkv"));
        let wide = movie(1, &"\u{65e5}".repeat(100), Some(2000), None);
        let (path, _) = movie_path("/srv/media/Movies", &wide, "1080p", None, None, &Settings::default());
        let folder = format!("{} (2000) {{tmdb-1}}", "\u{65e5}".repeat(75));
        assert_eq!(path.components, [folder.clone(), format!("{folder} - 1080p.mkv")]);
        assert_eq!(path.components[1].len(), 253);
    }

    /// The owner's TV library root (a UNC path).
    const SHOWS: &str = r"\\unas-pro\media\Shows";

    #[test]
    fn an_episode_path_is_the_show_folder_the_season_folder_and_the_episode_file() {
        let (path, notes) = tv_path(SHOWS, &example_show(), 3, &[16], Some("First Day"), None, &Settings::default());
        assert_eq!(
            path,
            NamedPath {
                root: SHOWS.to_string(),
                components: texts(&[
                    "Example Show (2005) {tmdb-4343}",
                    "Season 03",
                    "Example Show (2005) - S03E16 - First Day.mkv",
                ]),
                path: r"\\unas-pro\media\Shows\Example Show (2005) {tmdb-4343}\Season 03\Example Show (2005) - S03E16 - First Day.mkv"
                    .to_string(),
            }
        );
        assert!(notes.is_empty());
    }

    #[test]
    fn a_multi_episode_title_names_its_first_and_last_episode() {
        let name = Some("Part One/Part Two");
        let (path, _) = tv_path(SHOWS, &example_show(), 3, &[12, 13], name, None, &Settings::default());
        assert_eq!(
            parts(&path),
            [
                "Example Show (2005) {tmdb-4343}",
                "Season 03",
                "Example Show (2005) - S03E12-E13 - Part OnePart Two.mkv",
            ]
        );
    }

    #[test]
    fn three_episodes_are_named_as_a_range_whatever_their_order() {
        let settings = Settings::default();
        let (three, _) = tv_path(SHOWS, &example_show(), 3, &[3, 1, 2], None, None, &settings);
        assert_eq!(three.components[2], "Example Show (2005) - S03E01-E03.mkv");
        let (reversed, _) = tv_path(SHOWS, &example_show(), 3, &[13, 12], None, None, &settings);
        assert_eq!(reversed.components[2], "Example Show (2005) - S03E12-E13.mkv");
        let (repeated, _) = tv_path(SHOWS, &example_show(), 3, &[14, 14], None, None, &settings);
        assert_eq!(repeated.components[2], "Example Show (2005) - S03E14.mkv");
    }

    #[test]
    fn specials_go_in_season_00() {
        let (path, _) = tv_path(SHOWS, &example_show(), 0, &[1], Some("Holiday Special"), None, &Settings::default());
        assert_eq!(
            parts(&path),
            ["Example Show (2005) {tmdb-4343}", "Season 00", "Example Show (2005) - S00E01 - Holiday Special.mkv"]
        );
    }

    #[test]
    fn without_an_episode_title_the_file_ends_at_the_episode_code() {
        let (path, _) = tv_path(SHOWS, &example_show(), 3, &[16], None, None, &Settings::default());
        assert_eq!(path.components[2], "Example Show (2005) - S03E16.mkv");
    }

    #[test]
    fn an_episode_title_that_sanitises_to_nothing_is_left_out() {
        for name in ["???", "   ", "..."] {
            let (path, _) = tv_path(SHOWS, &example_show(), 3, &[16], Some(name), None, &Settings::default());
            assert_eq!(path.components[2], "Example Show (2005) - S03E16.mkv", "{name:?}");
        }
    }

    #[test]
    fn the_episode_title_is_capped_at_90_characters_in_the_file_name() {
        let long = "The One Where Everybody Finds Out About the Very Long Episode Title That Goes On and On Without Ever Stopping";
        let (path, _) = tv_path(SHOWS, &example_show(), 3, &[16], Some(long), None, &Settings::default());
        assert_eq!(
            path.components[2],
            "Example Show (2005) - S03E16 - The One Where Everybody Finds Out About the Very Long Episode Title That Goes On and On Wi.mkv"
        );
    }

    #[test]
    fn the_copy_label_follows_the_episode_code() {
        let settings = Settings::default();
        let (single, _) = tv_path(SHOWS, &example_show(), 3, &[16], Some("First Day"), Some(2), &settings);
        assert_eq!(single.components[2], "Example Show (2005) - S03E16 2 - First Day.mkv");
        let name = Some("Part One/Part Two");
        let (double, _) = tv_path(SHOWS, &example_show(), 3, &[12, 13], name, Some(3), &settings);
        assert_eq!(double.components[2], "Example Show (2005) - S03E12-E13 3 - Part OnePart Two.mkv");
    }

    #[test]
    fn the_compat_tag_is_on_the_show_folder_only() {
        let (path, notes) = tv_path(SHOWS, &example_show(), 3, &[16], Some("First Day"), None, &compat_on());
        assert_eq!(
            parts(&path),
            [
                "Example Show (2005) {tmdb-4343} [tmdbid-4343]",
                "Season 03",
                "Example Show (2005) - S03E16 - First Day.mkv",
            ]
        );
        assert!(notes.is_empty());
    }

    #[test]
    fn a_year_like_show_id_takes_the_tvdbid_tag_on_the_show_folder() {
        let work = show(2010, "Show", Some(2008), None, Some(81189));
        let (path, notes) = tv_path(SHOWS, &work, 1, &[1], Some("Pilot"), None, &compat_on());
        assert_eq!(
            parts(&path),
            ["Show (2008) {tmdb-2010} [tvdbid-81189]", "Season 01", "Show (2008) - S01E01 - Pilot.mkv"]
        );
        assert!(notes.is_empty());
    }

    #[test]
    fn a_year_like_show_id_without_a_tvdb_id_omits_the_tag_with_a_note() {
        let work = show(2010, "Show", Some(2008), Some("tt7654321"), None);
        let (path, notes) = tv_path(SHOWS, &work, 1, &[1], Some("Pilot"), None, &compat_on());
        assert_eq!(parts(&path), ["Show (2008) {tmdb-2010}", "Season 01", "Show (2008) - S01E01 - Pilot.mkv"]);
        assert_eq!(
            notes,
            vec![Noted {
                note: Note::CompatTagOmitted,
                stream: None,
                detail: "TMDb ID 2010 reads as a year to Jellyfin 10.x and TMDb gives no TVDB ID: the compatibility tag is left out"
                    .to_string(),
            }]
        );
        let (_, off) = tv_path(SHOWS, &work, 1, &[1], Some("Pilot"), None, &Settings::default());
        assert!(off.is_empty());
    }

    #[test]
    fn a_show_without_a_year_writes_no_year() {
        let undated = show(4343, "Example Show", None, Some("tt0004343"), Some(70001));
        let (path, _) = tv_path(SHOWS, &undated, 3, &[16], Some("First Day"), None, &Settings::default());
        assert_eq!(
            parts(&path),
            ["Example Show {tmdb-4343}", "Season 03", "Example Show - S03E16 - First Day.mkv"]
        );
    }

    #[test]
    fn an_empty_episode_list_writes_the_season_code_alone() {
        let (path, _) = tv_path(SHOWS, &example_show(), 3, &[], None, None, &Settings::default());
        assert_eq!(path.components[2], "Example Show (2005) - S03.mkv");
    }

    #[test]
    fn a_long_show_title_keeps_the_show_tag_and_the_episode_code() {
        let long = show(4343, &"B".repeat(300), Some(2005), None, None);
        let name = Some("First Day");
        let (path, _) = tv_path("/srv/media/Shows", &long, 3, &[16], name, None, &Settings::default());
        let show_name = "B".repeat(235);
        assert_eq!(
            path.components,
            [
                format!("{show_name} (2005) {{tmdb-4343}}"),
                "Season 03".to_string(),
                format!("{show_name} (2005) - S03E16.mkv"),
            ]
        );
        assert_eq!(path.components[0].len(), 254);
        assert_eq!(path.components[2].len(), 255);
    }

    #[test]
    fn a_named_path_serialises_with_its_field_order_and_round_trips() {
        let (path, _) = tv_path("/srv/media/Shows", &example_show(), 3, &[16], Some("First Day"), None, &Settings::default());
        let text = serde_json::to_string(&path).unwrap();
        assert_eq!(
            text,
            r#"{"root":"/srv/media/Shows","components":["Example Show (2005) {tmdb-4343}","Season 03","Example Show (2005) - S03E16 - First Day.mkv"],"path":"/srv/media/Shows/Example Show (2005) {tmdb-4343}/Season 03/Example Show (2005) - S03E16 - First Day.mkv"}"#
        );
        assert_eq!(serde_json::from_str::<NamedPath>(&text).unwrap(), path);
    }
}
