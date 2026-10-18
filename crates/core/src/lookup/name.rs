//! The disc's own name (spec 8.1: "The disc's own name is the `bdmt` disc name, else the volume label", the season
//! and disc numbers parsed from it before cleaning, its cleaning, the generic names that are never searched, and the
//! plausibility of a TMDb search result) and spec 8.2's season marker. Defined by Task 4; used by `lookup::identify`
//! (Task 4), `plan::kind::disc_kind` (Task 5), `plan::tv` (Task 6), `plan::plan` (Task 7) and `lazymkv-dev plan`
//! (Task 15).
//!
//! A name is read in tokens, parts and pieces. The tokens are its words once underscores are spaces (split on
//! whitespace); each token splits on hyphens into parts; each part splits into pieces, the maximal runs of ASCII
//! digits and of other characters. `EXAMPLE SHOW S3D3` has the pieces `EXAMPLE`, `SHOW`, `S`, `3`, `D`, `3`, and `Pan's`
//! is one piece. A season marker is a piece `season`, `series` or `s` (any case) followed by a digit piece; a disc
//! number is a piece `disc` or `d` followed by a digit piece. Cleaning drops a part whose every piece belongs to a
//! marker, and the region and format tags.

use std::collections::BTreeSet;

use crate::lookup::tmdb::TmdbSearchResult;

/// One piece of a name (see the module doc): a run of ASCII digits or a run of other characters, inside one hyphen
/// part of one token.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Piece {
    /// The index of the token (a whitespace-separated word once underscores are spaces).
    token: usize,
    /// The index of the hyphen-separated part within that token.
    part: usize,
    text: String,
    digits: bool,
}

/// The tokens of a name: underscores read as spaces (spec 8.1 "Cleaning replaces underscores with spaces"), then split
/// on whitespace.
fn tokens(raw: &str) -> Vec<String> {
    raw.replace('_', " ").split_whitespace().map(str::to_string).collect()
}

/// Every piece of `tokens`, in order.
fn pieces(tokens: &[String]) -> Vec<Piece> {
    let mut found = Vec::new();
    for (token, word) in tokens.iter().enumerate() {
        for (part, text) in word.split('-').enumerate() {
            let mut run = String::new();
            let mut run_digits = false;
            for character in text.chars() {
                let digit = character.is_ascii_digit();
                if !run.is_empty() && digit != run_digits {
                    found.push(Piece { token, part, text: std::mem::take(&mut run), digits: run_digits });
                }
                run_digits = digit;
                run.push(character);
            }
            if !run.is_empty() {
                found.push(Piece { token, part, text: run, digits: run_digits });
            }
        }
    }
    found
}

/// What a marker numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MarkerKind {
    Season,
    Disc,
}

/// A season marker or a disc number: its number, and the indices of its word piece and its digit piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Marker {
    kind: MarkerKind,
    number: u32,
    word: usize,
    digits: usize,
}

/// The words that start a season marker (spec 8.2 "`season`, `series` or `s` followed by a number").
const SEASON_WORDS: [&str; 3] = ["season", "series", "s"];
/// The words that start a disc number (spec 8.1 "`DISC 1`, `D1`").
const DISC_WORDS: [&str; 2] = ["disc", "d"];

/// Every marker of `pieces`, left to right; a digit piece belongs to one marker at most.
fn markers(pieces: &[Piece]) -> Vec<Marker> {
    let mut found = Vec::new();
    let mut index = 0;
    while index + 1 < pieces.len() {
        let (word, next) = (&pieces[index], &pieces[index + 1]);
        if !word.digits && next.digits {
            let lower = word.text.to_ascii_lowercase();
            let kind = if SEASON_WORDS.contains(&lower.as_str()) {
                Some(MarkerKind::Season)
            } else if DISC_WORDS.contains(&lower.as_str()) {
                Some(MarkerKind::Disc)
            } else {
                None
            };
            if let (Some(kind), Ok(number)) = (kind, next.text.parse::<u32>()) {
                found.push(Marker { kind, number, word: index, digits: index + 1 });
                index += 2;
                continue;
            }
        }
        index += 1;
    }
    found
}

/// The number of the first marker of `kind` in `raw`.
fn first_marker(raw: &str, kind: MarkerKind) -> Option<u32> {
    markers(&pieces(&tokens(raw))).into_iter().find(|marker| marker.kind == kind).map(|marker| marker.number)
}

/// A season number from `season N`, `series N`, `sN` or `sNN` (case-insensitive; `S3D3` gives 3; tokens split on
/// spaces, underscores, hyphens and the letter boundary of `S3D3`). Spec 8.1 "a season number and a disc number are
/// parsed". Def: T4. Use: T4.
pub fn parse_season(raw: &str) -> Option<u32> {
    first_marker(raw, MarkerKind::Season)
}

/// A disc number from `disc N`, `disc_N`, `discN`, `dN`, `D1` (`CHERNOBYL_D1` gives 1, `S3D3` gives 3). Spec 8.1.
/// Def: T4. Use: T4.
pub fn parse_disc_number(raw: &str) -> Option<u32> {
    first_marker(raw, MarkerKind::Disc)
}

/// Spec 8.2: the name contains `season`, `series` or `s` followed by a number (as `parse_season` finds it). A disc
/// number alone never counts. Def: T4. Use: T4, T5.
pub fn has_season_marker(raw: &str) -> bool {
    parse_season(raw).is_some()
}

/// The region and format tags spec 8.1's cleaning removes ("region and format tags"), upper-case.
const TAGS: [&str; 24] = [
    "UHD", "4K", "BD", "BD50", "BLURAY", "BLU-RAY", "BDROM", "DVD", "DVD9", "NTSC", "PAL", "US", "UK", "EU", "WW", "DOM",
    "INT", "R1", "R2", "R3", "R4", "R5", "R6", "RETAIL",
];

/// `token` without surrounding round or square brackets (`(UHD)`, `[2006]`).
fn unbracketed(token: &str) -> &str {
    token.trim_matches(|character: char| matches!(character, '(' | ')' | '[' | ']'))
}

/// A region or format tag, in any case, with or without brackets.
fn is_tag(token: &str) -> bool {
    TAGS.contains(&unbracketed(token).to_ascii_uppercase().as_str())
}

/// A year from 1900 to 2099, with or without brackets (spec 8.1 "trailing years").
fn is_year(token: &str) -> bool {
    let digits = unbracketed(token);
    digits.len() == 4
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(digits.parse::<u32>(), Ok(1900..=2099))
}

/// Spec 8.1 "Cleaning": underscores to spaces; disc numbers (`DISC 1`, `D1`, `DISC_1`), season markers (`SEASON 3`,
/// `S3`, `S3D3`, `SERIES 2`), region and format tags (whole tokens, case-insensitive: UHD, 4K, BD, BD50, BLURAY,
/// BLU-RAY, BDROM, DVD, DVD9, NTSC, PAL, US, UK, EU, WW, DOM, INT, R1 to R6, RETAIL) and a trailing year (1900 to 2099
/// as the last token) removed; repeated spaces collapsed; trimmed. A bare number that is not a tag stays
/// (`EXPENDABLES_3` gives `EXPENDABLES 3`). A marker or a tag that is a whole hyphen part goes too (`MOVIE-D2` gives
/// `MOVIE`, `X-MEN-UHD` gives `X-MEN`); a tag may sit in round or square brackets; a year that is the only token left
/// stays. Def: T4. Use: T4.
pub fn clean(raw: &str) -> String {
    let tokens = tokens(raw);
    let pieces = pieces(&tokens);
    let in_marker: BTreeSet<usize> =
        markers(&pieces).iter().flat_map(|marker| [marker.word, marker.digits]).collect();
    let mut kept: Vec<String> = Vec::new();
    for (token, word) in tokens.iter().enumerate() {
        if is_tag(word) {
            continue;
        }
        let mut parts: Vec<&str> = Vec::new();
        for (part, text) in word.split('-').enumerate() {
            let of_part: Vec<usize> = pieces
                .iter()
                .enumerate()
                .filter(|(_, piece)| piece.token == token && piece.part == part)
                .map(|(index, _)| index)
                .collect();
            let marker_only = !of_part.is_empty() && of_part.iter().all(|index| in_marker.contains(index));
            if !marker_only && !is_tag(text) {
                parts.push(text);
            }
        }
        let joined = parts.join("-");
        if !joined.is_empty() {
            kept.push(joined);
        }
    }
    if kept.len() > 1 && kept.last().is_some_and(|last| is_year(last)) {
        kept.pop();
    }
    kept.join(" ")
}

/// Spec 8.1's generic or placeholder names ("for example `DVD_VIDEO`, `DISC_1`, `LOGICAL_VOLUME_ID`, `VOLUME_SET_ID`")
/// and the other placeholders a disc carries, compared upper-case with spaces read as underscores. Def: T4. Use: T4.
pub const GENERIC_NAMES: [&str; 8] =
    ["DVD_VIDEO", "DISC_1", "LOGICAL_VOLUME_ID", "VOLUME_SET_ID", "BLURAY", "BDROM", "UNTITLED", "UNDEFINED"];

/// Spec 8.1: `DVD_VIDEO`, `DISC_1`, `LOGICAL_VOLUME_ID`, `VOLUME_SET_ID`, `BLURAY`, `BDROM`, `UNTITLED`, `UNDEFINED`, a
/// cleaned name that is empty, or one with no letter. The raw name is compared upper-case, its underscores and spaces
/// read alike (`dvd video` is `DVD_VIDEO`); a generic name is never searched (identity step 6). Def: T4. Use: T4.
pub fn is_generic(raw: &str, cleaned: &str) -> bool {
    let key = tokens(raw).join("_").to_ascii_uppercase();
    GENERIC_NAMES.contains(&key.as_str()) || !cleaned.chars().any(char::is_alphabetic)
}

/// Case-folded (Unicode lowercase), every character that is not alphanumeric or a space removed, spaces collapsed and
/// trimmed (`Pan's Labyrinth` and `PANS LABYRINTH` both give `pans labyrinth`). Spec 8.1 "after case-folding and
/// removing punctuation". Any whitespace counts as a space; accents stay (`Amélie` gives `amélie`). It does not call
/// `subtitle::text::fold` (ASCII folding for the text rule, other semantics). Def: T4. Use: T4.
pub fn fold_name(text: &str) -> String {
    let kept: String = text
        .to_lowercase()
        .chars()
        .map(|character| if character.is_whitespace() { ' ' } else { character })
        .filter(|character| character.is_alphanumeric() || *character == ' ')
        .collect();
    kept.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// A result is plausible when `fold_name` of its title or original title equals `fold_name(cleaned)` (spec 8.1 "A TMDb
/// search result is plausible when its title or original title equals the cleaned name after case-folding and
/// removing punctuation"); a cleaned name that folds to nothing makes no result plausible. `display_title` and
/// `original` give `name` and `original_name` for a TV result. Def: T4. Use: T4.
pub fn plausible(result: &TmdbSearchResult, cleaned: &str) -> bool {
    let wanted = fold_name(cleaned);
    !wanted.is_empty() && (fold_name(result.display_title()) == wanted || fold_name(result.original()) == wanted)
}

/// Where the disc's own name came from (8.1 "the bdmt disc name, else the volume label"). Def: T4. Use: T4, T7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameSource {
    /// The `<di:name>` of `BDMV/META/DL/bdmt_*.xml` (6.1 step 2; `disc::bdmt_name` reads it).
    Bdmt,
    /// CINFO 32, the volume label.
    VolumeLabel,
}

impl NameSource {
    /// The serde spelling: "bdmt" or "volume_label". Spec 8.1.
    pub fn as_str(self) -> &'static str {
        match self {
            NameSource::Bdmt => "bdmt",
            NameSource::VolumeLabel => "volume_label",
        }
    }

    /// Both sources, in 8.1's order of preference. Spec 8.1.
    pub const ALL: [NameSource; 2] = [NameSource::Bdmt, NameSource::VolumeLabel];
}

impl std::str::FromStr for NameSource {
    type Err = String;

    /// Only the `as_str` spellings.
    fn from_str(text: &str) -> Result<NameSource, String> {
        NameSource::ALL
            .into_iter()
            .find(|source| source.as_str() == text)
            .ok_or_else(|| format!("'{text}' is not a name source (bdmt or volume_label)"))
    }
}

/// The disc's own name with the evidence parsed from it before cleaning. Spec 8.1, 8.2. Def: T4. Use: T4 (identify),
/// T5 (season marker), T6 (season), T7.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscName {
    /// The name as given, trimmed.
    pub raw: String,
    pub source: NameSource,
    /// `parse_season(raw)`.
    pub season: Option<u32>,
    /// `parse_disc_number(raw)`.
    pub disc: Option<u32>,
    /// `clean(raw)`.
    pub cleaned: String,
    /// `is_generic(raw, cleaned)`.
    pub generic: bool,
    /// `has_season_marker(raw)` (8.2).
    pub season_marker: bool,
}

/// `text` trimmed, when Some and not blank.
fn given(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

/// The bdmt name when Some and not blank, else the volume label when Some and not blank, else None. The caller builds
/// it once (`disc_name(bdmt, disc.volume_label.as_deref())`), searches TMDb with its `cleaned`, and passes it as
/// `LookupInputs.name`; `plan::plan` reads it from there and copies it into `PlanEvidence.name`. Spec 8.1 "The disc's
/// own name is the `bdmt` disc name, else the volume label". Def: T4. Use: T7 (tests), T15.
pub fn disc_name(bdmt: Option<&str>, volume_label: Option<&str>) -> Option<DiscName> {
    let (raw, source) = match (given(bdmt), given(volume_label)) {
        (Some(name), _) => (name, NameSource::Bdmt),
        (None, Some(label)) => (label, NameSource::VolumeLabel),
        (None, None) => return None,
    };
    let cleaned = clean(raw);
    Some(DiscName {
        raw: raw.to_string(),
        source,
        season: parse_season(raw),
        disc: parse_disc_number(raw),
        generic: is_generic(raw, &cleaned),
        season_marker: has_season_marker(raw),
        cleaned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_season_number_is_parsed_from_every_spelling() {
        assert_eq!(parse_season("EXAMPLE SHOW S3D3"), Some(3));
        assert_eq!(parse_season("FRIENDS_SEASON_1"), Some(1));
        assert_eq!(parse_season("Friends Season 10 Disc 2"), Some(10));
        assert_eq!(parse_season("season3"), Some(3));
        assert_eq!(parse_season("BLACKADDER SERIES 2"), Some(2));
        assert_eq!(parse_season("SHOW_S03"), Some(3));
        assert_eq!(parse_season("show s1 d2"), Some(1));
        assert_eq!(parse_season("SHOW-S2"), Some(2));
    }

    #[test]
    fn a_name_without_a_season_marker_has_no_season() {
        for raw in [
            "CHERNOBYL_D1",
            "Chernobyl Disc 1",
            "EXPENDABLES_3",
            "Pan's Labyrinth",
            "Ocean's 11",
            "THE COMPLETE SERIES DISC 1",
            "SMILE",
            "",
        ] {
            assert_eq!(parse_season(raw), None, "{raw}");
        }
    }

    #[test]
    fn a_disc_number_is_parsed_from_every_spelling() {
        assert_eq!(parse_disc_number("CHERNOBYL_D1"), Some(1));
        assert_eq!(parse_disc_number("Chernobyl Disc 2"), Some(2));
        assert_eq!(parse_disc_number("FIREFLY_DISC_3"), Some(3));
        assert_eq!(parse_disc_number("firefly disc4"), Some(4));
        assert_eq!(parse_disc_number("EXAMPLE SHOW S3D3"), Some(3));
        assert_eq!(parse_disc_number("SHOW_d02"), Some(2));
        for raw in ["EXPENDABLES_3", "JOHN_WICK_UHD", "BLACKADDER SERIES 2", "DVD9", "R2", ""] {
            assert_eq!(parse_disc_number(raw), None, "{raw}");
        }
    }

    #[test]
    fn a_disc_number_alone_is_not_a_season_marker() {
        assert!(has_season_marker("EXAMPLE SHOW S3D3"));
        assert!(has_season_marker("FRIENDS_SEASON_1"));
        assert!(has_season_marker("BLACKADDER SERIES 2"));
        assert!(!has_season_marker("CHERNOBYL_D1"));
        assert!(!has_season_marker("FIREFLY_DISC_1"));
        assert!(!has_season_marker("EXPENDABLES_3"));
        assert!(!has_season_marker("SMILE"));
    }

    #[test]
    fn a_number_too_large_for_a_marker_is_not_one() {
        assert_eq!(parse_season("SHOW S99999999999"), None);
        assert_eq!(parse_disc_number("SHOW D99999999999"), None);
    }

    /// The volume label of each pinned log and recording, and its cleaned form (spec 8.1 "Cleaning").
    const THIRTEEN: [(&str, &str); 13] = [
        ("JOHN_WICK_UHD", "JOHN WICK"),
        ("JOHN_WICK", "JOHN WICK"),
        ("KNOWING", "KNOWING"),
        ("SMILE", "SMILE"),
        ("EXPENDABLES_3", "EXPENDABLES 3"),
        ("CHERNOBYL_D1", "CHERNOBYL"),
        ("CHERNOBYL_D2", "CHERNOBYL"),
        ("FIREFLY_D1", "FIREFLY"),
        ("FIREFLY_D2", "FIREFLY"),
        ("PANS_LABYRINTH", "PANS LABYRINTH"),
        ("EXAMPLE SHOW S3D3", "EXAMPLE SHOW"),
        ("EXAMPLE_FILM", "EXAMPLE FILM"),
        ("DVD_VIDEO", "VIDEO"),
    ];

    #[test]
    fn the_thirteen_recorded_and_pinned_names_clean_to_these() {
        for (raw, cleaned) in THIRTEEN {
            assert_eq!(clean(raw), cleaned, "{raw}");
        }
    }

    #[test]
    fn the_pinned_logs_volume_labels_and_names_clean_as_listed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb");
        let logs = [
            ("movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt", "JOHN_WICK_UHD", "JOHN_WICK_UHD", "JOHN WICK", "JOHN WICK"),
            ("movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc02.txt", "JOHN_WICK", "JOHN_WICK", "JOHN WICK", "JOHN WICK"),
            ("movie/Knowing (2009)/2010-dvd/disc01.txt", "KNOWING", "KNOWING", "KNOWING", "KNOWING"),
            ("movie/Smile (2022)/2022-4k/disc01.txt", "SMILE", "Smile", "SMILE", "Smile"),
            (
                "movie/The Expendables 3 (2014)/the-expendables-3-film-collection/disc01.txt",
                "EXPENDABLES_3",
                "Expendables 3",
                "EXPENDABLES 3",
                "Expendables 3",
            ),
            ("series/Chernobyl (2019)/2020-4k-US/disc01.txt", "CHERNOBYL_D1", "Chernobyl Disc 1", "CHERNOBYL", "Chernobyl"),
            ("series/Chernobyl (2019)/2020-4k-US/disc02.txt", "CHERNOBYL_D2", "Chernobyl Disc 2", "CHERNOBYL", "Chernobyl"),
            ("series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt", "FIREFLY_D1", "FIREFLY_D1", "FIREFLY", "FIREFLY"),
            ("series/Firefly (2002)/2003-dvd-the-complete-series/disc02.txt", "FIREFLY_D2", "FIREFLY_D2", "FIREFLY", "FIREFLY"),
        ];
        for (relative, label, name, cleaned_label, cleaned_name) in logs {
            let text = std::fs::read_to_string(root.join(relative)).unwrap_or_else(|error| panic!("{relative}: {error}"));
            let scan = crate::robot::parse(&text);
            assert_eq!(scan.disc.get(&32).map(String::as_str), Some(label), "{relative}: CINFO 32");
            assert_eq!(scan.disc.get(&2).map(String::as_str), Some(name), "{relative}: CINFO 2");
            assert_eq!(clean(label), cleaned_label, "{relative}");
            assert_eq!(clean(name), cleaned_name, "{relative}");
            assert_eq!(
                THIRTEEN.iter().find(|(raw, _)| *raw == label).map(|(_, cleaned)| *cleaned),
                Some(cleaned_label),
                "{relative}"
            );
        }
    }

    #[test]
    fn the_bdmt_and_scan_names_clean_too() {
        assert_eq!(clean("Pan's Labyrinth"), "Pan's Labyrinth");
        assert_eq!(clean("Example Film 2"), "Example Film 2");
        assert_eq!(clean("Chernobyl Disc 1"), "Chernobyl");
        assert_eq!(clean("Expendables 3"), "Expendables 3");
    }

    #[test]
    fn underscores_become_spaces_and_repeated_spaces_collapse() {
        assert_eq!(clean("  EXAMPLE__SHOW   S3D3 "), "EXAMPLE SHOW");
        assert_eq!(clean("A__B"), "A B");
        assert_eq!(clean("PANS_LABYRINTH"), "PANS LABYRINTH");
        assert_eq!(clean(""), "");
    }

    #[test]
    fn disc_numbers_are_removed_in_every_spelling() {
        for raw in ["MOVIE DISC 1", "MOVIE_D1", "MOVIE_DISC_1", "MOVIE disc2", "MOVIE-D2", "MOVIE d02", "DISC 1 MOVIE"] {
            assert_eq!(clean(raw), "MOVIE", "{raw}");
        }
    }

    #[test]
    fn season_markers_are_removed_in_every_spelling() {
        for raw in [
            "SHOW SEASON 3",
            "SHOW_S3",
            "SHOW S03",
            "SHOW SERIES 2",
            "SHOW_S3D3",
            "SHOW S1 D2",
            "SHOW-S2",
            "SHOW season3",
            "SHOW_SEASON_1_DISC_2",
        ] {
            assert_eq!(clean(raw), "SHOW", "{raw}");
        }
    }

    #[test]
    fn every_region_and_format_tag_is_removed_as_a_whole_token_in_any_case() {
        let tags = [
            "UHD", "4K", "BD", "BD50", "BLURAY", "BLU-RAY", "BDROM", "DVD", "DVD9", "NTSC", "PAL", "US", "UK", "EU", "WW",
            "DOM", "INT", "R1", "R2", "R3", "R4", "R5", "R6", "RETAIL",
        ];
        for tag in tags {
            assert_eq!(clean(&format!("NAME_{tag}")), "NAME", "{tag}");
            assert_eq!(clean(&format!("name {}", tag.to_lowercase())), "name", "{tag} lower-case");
            assert_eq!(clean(&format!("NAME ({tag})")), "NAME", "{tag} in brackets");
        }
        assert_eq!(clean("NAME [4k]"), "NAME");
        assert_eq!(clean("X-MEN-UHD"), "X-MEN");
        assert_eq!(clean("Smile_WW"), "Smile");
        // a tag token is removed wherever it sits, so a real word such as PAL or US is lost (listed for the owner)
        assert_eq!(clean("THE UK FILES"), "THE FILES");
        assert_eq!(clean("MY_PAL_GUS"), "MY GUS");
        assert_eq!(clean("US"), "");
    }

    #[test]
    fn a_tag_inside_a_word_stays() {
        assert_eq!(clean("PALADIN"), "PALADIN");
        assert_eq!(clean("THE USUAL SUSPECTS"), "THE USUAL SUSPECTS");
        assert_eq!(clean("DVDS"), "DVDS");
        assert_eq!(clean("UHDTV"), "UHDTV");
        assert_eq!(clean("Spider-Man"), "Spider-Man");
    }

    #[test]
    fn a_trailing_year_is_removed_but_not_a_leading_or_a_sole_one() {
        assert_eq!(clean("ALIEN 1979"), "ALIEN");
        assert_eq!(clean("ALIEN_(1979)"), "ALIEN");
        assert_eq!(clean("ALIEN 1979 UHD"), "ALIEN");
        assert_eq!(clean("2001 A SPACE ODYSSEY"), "2001 A SPACE ODYSSEY");
        assert_eq!(clean("1917"), "1917");
        assert_eq!(clean("ALIEN 1899"), "ALIEN 1899");
        assert_eq!(clean("ALIEN 2100"), "ALIEN 2100");
        // Spec 8.1's rule read literally: a title that ends in a year loses it (listed for the owner).
        assert_eq!(clean("BLADE_RUNNER_2049"), "BLADE RUNNER");
    }

    #[test]
    fn a_bare_number_that_is_not_a_tag_or_a_marker_stays() {
        assert_eq!(clean("EXPENDABLES_3"), "EXPENDABLES 3");
        assert_eq!(clean("OCEANS_11"), "OCEANS 11");
        assert_eq!(clean("Ocean's 8"), "Ocean's 8");
        assert_eq!(clean("TOY STORY 3 UHD"), "TOY STORY 3");
    }

    #[test]
    fn a_3d_label_s3d_reads_as_season_3_by_spec_8_2s_literal_rule() {
        // `S3D` (stereoscopic 3D) is the pieces `S`, `3`, `D`: spec 8.2's season marker read literally, so the movie
        // disc has a season marker, and cleaning keeps `S3D` because its `D` piece belongs to no marker (listed for the
        // owner; treating `S3D` and `3D` as format tags is the alternative).
        assert_eq!(parse_season("HUGO_S3D"), Some(3));
        assert!(has_season_marker("HUGO_S3D"));
        assert_eq!(clean("HUGO_S3D"), "HUGO S3D");
    }

    #[test]
    fn the_generic_names_are_spec_8_1s_list() {
        assert_eq!(
            GENERIC_NAMES,
            ["DVD_VIDEO", "DISC_1", "LOGICAL_VOLUME_ID", "VOLUME_SET_ID", "BLURAY", "BDROM", "UNTITLED", "UNDEFINED"]
        );
    }

    #[test]
    fn every_generic_name_is_generic_whatever_its_case_or_separator() {
        for name in GENERIC_NAMES {
            for raw in [name.to_string(), name.to_lowercase(), name.replace('_', " "), format!(" {name} ")] {
                assert!(is_generic(&raw, &clean(&raw)), "{raw:?}");
            }
        }
    }

    #[test]
    fn a_name_with_no_letters_or_nothing_left_after_cleaning_is_generic() {
        for raw in ["12345", "2012", "DISC_2", "BLU-RAY", "UHD 4K", "S3D3", "   ", ""] {
            assert!(is_generic(raw, &clean(raw)), "{raw:?}");
        }
        assert!(is_generic("KNOWING", ""));
        // a disc labelled US (the film Us) cleans to nothing, so it is generic and never searched (listed for the owner)
        assert!(is_generic("US", &clean("US")));
    }

    #[test]
    fn dvd_video_is_generic_by_its_raw_name_though_it_cleans_to_video() {
        assert_eq!(clean("DVD_VIDEO"), "VIDEO");
        assert!(is_generic("DVD_VIDEO", "VIDEO"));
        assert!(!is_generic("VIDEO", "VIDEO"));
    }

    #[test]
    fn the_twelve_named_discs_are_not_generic() {
        for (raw, cleaned) in THIRTEEN.iter().filter(|(raw, _)| *raw != "DVD_VIDEO") {
            assert!(!is_generic(raw, cleaned), "{raw}");
        }
        assert!(!is_generic("Pan's Labyrinth", &clean("Pan's Labyrinth")));
        assert!(!is_generic("Example Film 2", &clean("Example Film 2")));
    }

    fn movie_result(id: i64, title: &str, original: &str) -> TmdbSearchResult {
        TmdbSearchResult {
            media_type: "movie".to_string(),
            id,
            title: Some(title.to_string()),
            name: None,
            original_title: Some(original.to_string()),
            original_name: None,
            release_date: Some("2006-10-11".to_string()),
            first_air_date: None,
        }
    }

    fn tv_result(id: i64, name: &str, original: &str) -> TmdbSearchResult {
        TmdbSearchResult {
            media_type: "tv".to_string(),
            id,
            title: None,
            name: Some(name.to_string()),
            original_title: None,
            original_name: Some(original.to_string()),
            release_date: None,
            first_air_date: Some("2005-03-24".to_string()),
        }
    }

    #[test]
    fn fold_name_case_folds_and_removes_punctuation() {
        assert_eq!(fold_name("Pan's Labyrinth"), "pans labyrinth");
        assert_eq!(fold_name("PANS LABYRINTH"), "pans labyrinth");
        assert_eq!(fold_name("John Wick: Chapter 2"), "john wick chapter 2");
        assert_eq!(fold_name("  The   Office! "), "the office");
        assert_eq!(fold_name("The\tOffice\n"), "the office");
        assert_eq!(fold_name("El laberinto del fauno"), "el laberinto del fauno");
        assert_eq!(fold_name("Spider-Man"), "spiderman");
        assert_eq!(fold_name("!!!"), "");
    }

    #[test]
    fn fold_name_lower_cases_beyond_ascii_and_keeps_accents() {
        assert_eq!(fold_name("ÉCOLE"), "école");
        assert_eq!(fold_name("Amélie"), "amélie");
        assert_ne!(fold_name("AMELIE"), fold_name("Amélie"));
    }

    #[test]
    fn a_result_is_plausible_by_its_title_or_its_original_title() {
        let result = movie_result(1417, "Pan's Labyrinth", "El laberinto del fauno");
        assert!(plausible(&result, "PANS LABYRINTH"));
        assert!(plausible(&result, "Pan's Labyrinth"));
        assert!(plausible(&result, "EL LABERINTO DEL FAUNO"));
    }

    #[test]
    fn a_tv_result_is_plausible_by_its_name_or_original_name() {
        let result = tv_result(2316, "The Office", "The Office");
        assert!(plausible(&result, "THE OFFICE"));
        let renamed = tv_result(9, "Le Bureau", "The Office");
        assert!(plausible(&renamed, "THE OFFICE"));
    }

    #[test]
    fn a_near_miss_is_not_plausible() {
        let result = movie_result(1417, "Pan's Labyrinth", "El laberinto del fauno");
        assert!(!plausible(&result, "PANS LABYRINTH 2"));
        assert!(!plausible(&result, "LABYRINTH"));
        assert!(!plausible(&tv_result(2316, "The Office", "The Office"), "THE OFFICE US"));
    }

    #[test]
    fn an_empty_cleaned_name_makes_nothing_plausible() {
        let untitled = TmdbSearchResult { title: None, original_title: None, ..movie_result(1, "", "") };
        assert!(!plausible(&untitled, ""));
        assert!(!plausible(&movie_result(2, "", ""), ""));
        assert!(!plausible(&movie_result(3, "!!!", "!!!"), "!!!"));
    }

    #[test]
    fn the_thirteen_names_give_this_evidence() {
        // (raw, cleaned, season, disc, generic, season marker)
        let expected: [(&str, &str, Option<u32>, Option<u32>, bool, bool); 13] = [
            ("JOHN_WICK_UHD", "JOHN WICK", None, None, false, false),
            ("JOHN_WICK", "JOHN WICK", None, None, false, false),
            ("KNOWING", "KNOWING", None, None, false, false),
            ("SMILE", "SMILE", None, None, false, false),
            ("EXPENDABLES_3", "EXPENDABLES 3", None, None, false, false),
            ("CHERNOBYL_D1", "CHERNOBYL", None, Some(1), false, false),
            ("CHERNOBYL_D2", "CHERNOBYL", None, Some(2), false, false),
            ("FIREFLY_D1", "FIREFLY", None, Some(1), false, false),
            ("FIREFLY_D2", "FIREFLY", None, Some(2), false, false),
            ("PANS_LABYRINTH", "PANS LABYRINTH", None, None, false, false),
            ("EXAMPLE SHOW S3D3", "EXAMPLE SHOW", Some(3), Some(3), false, true),
            ("EXAMPLE_FILM", "EXAMPLE FILM", None, None, false, false),
            ("DVD_VIDEO", "VIDEO", None, None, true, false),
        ];
        for (raw, cleaned, season, disc, generic, season_marker) in expected {
            assert_eq!(
                disc_name(None, Some(raw)),
                Some(DiscName {
                    raw: raw.to_string(),
                    source: NameSource::VolumeLabel,
                    season,
                    disc,
                    cleaned: cleaned.to_string(),
                    generic,
                    season_marker,
                }),
                "{raw}"
            );
        }
    }

    #[test]
    fn the_bdmt_name_wins_over_the_volume_label() {
        assert_eq!(
            disc_name(Some("Pan's Labyrinth"), Some("PANS_LABYRINTH")),
            Some(DiscName {
                raw: "Pan's Labyrinth".to_string(),
                source: NameSource::Bdmt,
                season: None,
                disc: None,
                cleaned: "Pan's Labyrinth".to_string(),
                generic: false,
                season_marker: false,
            })
        );
    }

    #[test]
    fn a_blank_name_falls_back_and_two_blank_names_give_none() {
        let label = disc_name(Some("   "), Some("PANS_LABYRINTH")).unwrap();
        assert_eq!((label.source, label.raw.as_str(), label.cleaned.as_str()), (NameSource::VolumeLabel, "PANS_LABYRINTH", "PANS LABYRINTH"));
        let bdmt = disc_name(Some("Pan's Labyrinth"), None).unwrap();
        assert_eq!(bdmt.source, NameSource::Bdmt);
        assert_eq!(disc_name(None, None), None);
        assert_eq!(disc_name(Some(""), Some("   ")), None);
    }

    #[test]
    fn the_name_is_trimmed_before_it_is_read() {
        let name = disc_name(None, Some("  CHERNOBYL_D1 ")).unwrap();
        assert_eq!((name.raw.as_str(), name.cleaned.as_str(), name.disc), ("CHERNOBYL_D1", "CHERNOBYL", Some(1)));
    }

    #[test]
    fn name_source_spellings_round_trip() {
        assert_eq!(NameSource::ALL, [NameSource::Bdmt, NameSource::VolumeLabel]);
        for (source, text) in [(NameSource::Bdmt, "bdmt"), (NameSource::VolumeLabel, "volume_label")] {
            assert_eq!(source.as_str(), text);
            assert_eq!(serde_json::to_string(&source).unwrap(), format!("\"{text}\""));
            assert_eq!(serde_json::from_str::<NameSource>(&format!("\"{text}\"")).unwrap(), source);
            assert_eq!(text.parse::<NameSource>(), Ok(source));
        }
        assert_eq!(
            "VolumeLabel".parse::<NameSource>(),
            Err("'VolumeLabel' is not a name source (bdmt or volume_label)".to_string())
        );
    }

    #[test]
    fn a_disc_name_serialises_with_its_fields_in_order() {
        let name = disc_name(None, Some("EXAMPLE SHOW S3D3")).unwrap();
        let text = serde_json::to_string(&name).unwrap();
        assert_eq!(
            text,
            r#"{"raw":"EXAMPLE SHOW S3D3","source":"volume_label","season":3,"disc":3,"cleaned":"EXAMPLE SHOW","generic":false,"season_marker":true}"#
        );
        assert_eq!(serde_json::from_str::<DiscName>(&text).unwrap(), name);
    }
}
