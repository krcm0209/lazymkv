//! Spec 8.3 rules 1 to 4, the movie titles, and 8.4, the cuts. Rule 1 takes TheDiscDB's main movies when every one
//! matches a scan title (0.98), and under OR5 adds each Java-labelled cut it does not list as an unnamed cut for review;
//! rule 2 reads the `FPL_MainFeature` labels a scan reports while Java is in use and plans one title per cut (0.90, or
//! 0.60 with `unnamed_cut`); rule 3 chooses among the groups of 8.2 by structure (play all, feature length, rivals,
//! chapters and stream supersets, segment difference: 0.85, or 0.60 with `title_ambiguous` or `multiple_features`);
//! rule 4 lowers a structural choice that is more than 10% from TMDb's runtime. `edition_names` applies 8.4's three
//! naming rules over `LibraryFacts`, the library facts phase 2 reads (RP18). Used by the planner (Task 7), naming
//! (Task 11) and the decision (Task 13).

use std::collections::{BTreeMap, BTreeSet};

use crate::disc::{Disc, StreamKind, Title};
use crate::lang::{is_english, Wanted, ISO639};
use crate::lookup::discdb::DiscDbRecord;
use crate::lookup::within;
use crate::review::{sorted_unique, Note, Noted, ReviewReason};
use crate::settings::Settings;

use super::kind::{candidate_filter, play_all_of_episodes, Group};

/// What every label starts with (8.3 rule 2), after an optional `SEG_` or `SEG `.
const LABEL_PREFIX: &str = "FPL_MainFeature";

/// The edition name of the theatrical or main cut (8.3 rule 2's table, 8.4). Def: T5. Use: T7, T13.
pub const THEATRICAL_CUT: &str = "Theatrical Cut";

/// 8.3 rule 2's suffix table. `Language` is a suffix that is only a language code (a dub variant of the cut without a
/// suffix); `Other` keeps the suffix as written. Def: T5. Use: T5.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Suffix {
    None,
    R,
    Ur,
    Ext,
    Narrative,
    Language(String),
    Other(String),
}

/// One parsed FPL_MainFeature label (8.3 rule 2): the scan title it is on, its suffix, the three-letter language code
/// split off its end (lower-cased), and whether a `Narrative` token followed another suffix (`_UR_Narrative`: the UR
/// cut's audio-description variant, ruling G5). Def: T5. Use: T5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub title: u32,
    pub suffix: Suffix,
    pub language: Option<String>,
    /// A trailing `Narrative` token after another suffix (G5); the bare `FPL_MainFeature_Narrative` is
    /// `Suffix::Narrative` with this false.
    pub narrative: bool,
}

/// A title comment (attribute 49) starting `FPL_MainFeature` (optionally preceded by `SEG_` or `SEG `): its suffix, with a
/// trailing `_XXX` three-letter language code split off as `language`, then a trailing `_Narrative` token (any ASCII
/// case) after another suffix split off as `narrative` (8.3 rule 2's table, ruling G5: `FPL_MainFeature_UR_Narrative`
/// is suffix UR with `narrative` true). None for any other comment. One `_` after the prefix is dropped; `R`, `UR`,
/// `EXT` and `Narrative` are compared ignoring ASCII case; a three-letter code counts when it is one of
/// `lang::ISO639`'s. Def: T5. Use: T5.
pub fn parse_label(title: u32, comment: &str) -> Option<Label> {
    let text = comment.strip_prefix("SEG_").or_else(|| comment.strip_prefix("SEG ")).unwrap_or(comment);
    let rest = text.strip_prefix(LABEL_PREFIX)?;
    let rest = rest.strip_prefix('_').unwrap_or(rest);
    let (body, language) = split_language(rest);
    let (body, narrative) = split_narrative(body);
    let suffix = match body.to_ascii_lowercase().as_str() {
        "" => match &language {
            Some(code) => Suffix::Language(code.clone()),
            None => Suffix::None,
        },
        "r" => Suffix::R,
        "ur" => Suffix::Ur,
        "ext" => Suffix::Ext,
        "narrative" => Suffix::Narrative,
        _ => Suffix::Other(body.to_string()),
    };
    Some(Label { title, suffix, language, narrative })
}

/// The suffix without a trailing `_Narrative` token (any ASCII case) and whether it had one (G5); the bare `Narrative`
/// suffix has no `_` before it and stays whole.
fn split_narrative(body: &str) -> (&str, bool) {
    match body.rfind('_') {
        Some(at) if body[at + 1..].eq_ignore_ascii_case("narrative") => (&body[..at], true),
        _ => (body, false),
    }
}

/// The suffix without its trailing language code, and that code lower-cased: the whole suffix when it is a code, else the
/// part after its last `_` when that is one.
fn split_language(rest: &str) -> (&str, Option<String>) {
    if is_language_code(rest) {
        return ("", Some(rest.to_ascii_lowercase()));
    }
    if let Some(at) = rest.rfind('_') {
        let code = &rest[at + 1..];
        if is_language_code(code) {
            return (&rest[..at], Some(code.to_ascii_lowercase()));
        }
    }
    (rest, None)
}

/// Three ASCII letters that, lower-cased, are one of the three-letter codes of `lang::ISO639`.
fn is_language_code(text: &str) -> bool {
    if text.len() != 3 || !text.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return false;
    }
    let code = text.to_ascii_lowercase();
    ISO639.iter().any(|(_, codes)| codes.contains(&code.as_str()))
}

/// The table's edition name: None and R "Theatrical Cut", Ur "Unrated Cut", Ext "Extended Cut"; Narrative and Language give
/// None (ignored or a variant), Other gives None (an unnamed cut). Def: T5. Use: T5.
pub fn edition_of(suffix: &Suffix) -> Option<&'static str> {
    match suffix {
        Suffix::None | Suffix::R => Some(THEATRICAL_CUT),
        Suffix::Ur => Some("Unrated Cut"),
        Suffix::Ext => Some("Extended Cut"),
        Suffix::Narrative | Suffix::Language(_) | Suffix::Other(_) => None,
    }
}

/// Which rule of 8.3 chose the features. Def: T5. Use: T7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleRule {
    Database,
    Labels,
    Structure,
}

impl TitleRule {
    /// The three rules, in 8.3's order.
    pub const ALL: [TitleRule; 3] = [TitleRule::Database, TitleRule::Labels, TitleRule::Structure];

    /// The serde spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            TitleRule::Database => "database",
            TitleRule::Labels => "labels",
            TitleRule::Structure => "structure",
        }
    }
}

impl std::str::FromStr for TitleRule {
    type Err = String;

    /// Only the `as_str` spellings.
    fn from_str(text: &str) -> Result<TitleRule, String> {
        TitleRule::ALL.into_iter().find(|rule| rule.as_str() == text).ok_or_else(|| format!("'{text}' is not a title rule"))
    }
}

/// Spec 8.3 rule 1: the disc database names the title, 0.98.
pub const DATABASE_TITLE_CONFIDENCE: f64 = 0.98;
/// Spec 8.3 rule 2: exactly one cut, or two or more cuts with different recognised suffixes, 0.90.
pub const LABELS_TITLE_CONFIDENCE: f64 = 0.90;
/// Spec 8.3 rule 3.5: one rival, which is the feature, 0.85.
pub const STRUCTURE_TITLE_CONFIDENCE: f64 = 0.85;
/// Spec 8.3: every ambiguous outcome (an unnamed cut, an ambiguous group, several rivals), 0.60.
pub const AMBIGUOUS_TITLE_CONFIDENCE: f64 = 0.60;
/// Spec 8.3 rule 3.3: a kept group within 10% of the longest kept group's duration is a rival.
pub const RIVAL_BAND: f64 = 0.10;
/// Spec 8.3 rule 3.6: rivals whose segment difference is at most 0.25 cannot be told apart.
pub const SEGMENT_DIFFERENCE_MAX: f64 = 0.25;
/// Spec 8.3 rule 4: a structural feature more than 10% from TMDb's runtime is in doubt.
pub const RUNTIME_SANITY_BAND: f64 = 0.10;

/// One feature or cut to rip. Spec 8.3, 8.4. Def: T5. Use: T7, T11, T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Feature {
    pub title: u32,
    /// The edition name as the evidence gives it (TheDiscDB's item title beyond the media title, else the label table, or
    /// None for the theatrical or only cut); `edition_names` turns it into the filed name.
    pub edition: Option<String>,
    /// True when `edition` is None or names the theatrical cut (contains the word Theatrical; `is_theatrical`), except an
    /// unnamed cut (rule 2's Other label suffix, a later TheDiscDB MainMovie with no edition, or a Java-labelled cut
    /// TheDiscDB does not list, OR5), which has edition None and theatrical false. A rule 1 main movie whose own label
    /// suffix is Other, or which carries no label, has edition None and stays theatrical unless it is a later main movie
    /// (c1:F1) or a cut TheDiscDB does not list is planned beside it, which leaves it unnamed too, so the owner names
    /// both (ruling H2). Spec 8.3, 8.4; rulings G9 and H2.
    pub theatrical: bool,
    /// The titles of its group (rule 3) or cut (rule 2) that were not chosen.
    pub siblings: Vec<u32>,
}

/// The movie titles of 8.3 with their confidence and reasons. Def: T5. Use: T7.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MoviePlan {
    pub rule: TitleRule,
    pub features: Vec<Feature>,
    /// Each feature's duration (`Title::seconds`), in feature order; rule 4 reads it, and the planner may fill
    /// `RuntimeFacts.feature_seconds` from it.
    pub feature_seconds: Vec<Option<u32>>,
    pub confidence: f64,
    pub reviews: Vec<ReviewReason>,
    pub notes: Vec<Noted>,
    /// Rule 3's rival groups (keys), for the evidence and the runtime facts.
    pub rivals: Vec<String>,
    pub evidence: Vec<String>,
}

/// Rule 1: applies only when TheDiscDB marks at least one MainMovie and every MainMovie title matches a scan title in
/// `matches`; one Feature per MainMovie, in database index order (0.98). Each Feature's edition is its item title with the
/// media title and surrounding brackets or dashes removed; when that leaves nothing, `disc.java_in_use` holds and the scan
/// title's comment (attribute 49) parses as a label, `edition_of(label.suffix)` (8.4 "else from the label table":
/// Expendables 3's title 41 `FPL_MainFeature_UR` gets "Unrated Cut"); `theatrical` as Feature's doc says. Two or more
/// MainMovies of which a later one still has no edition: 0.60 with unnamed_cut; two features given the same edition name
/// make it 0.60 with title_ambiguous (Review Focus 8). None when TheDiscDB marks no main movie or any MainMovie title does
/// not reconcile.
///
/// Ruling OR5 (2026-10-17) and OR16: a cut of rule 2's grouping (`by_labels(disc, wanted)`, so only when `disc.java_in_use`;
/// `wanted` is the languages `movie_plan` holds, SR3) none of whose titles, chosen or sibling, is a main movie's scan title
/// is a cut TheDiscDB does not list. Each is planned after the main movies, in rule 2's cut order, as an unnamed cut
/// (edition None, `theatrical` false, its chosen title the variant `wanted` prefers, its other titles its siblings), with
/// the review reason unnamed_cut, one note UnlistedLabelCut naming the title and its label as the scan wrote it, and one
/// evidence sentence, so the owner names it with Assign titles. The confidence stays what the main movies alone give (0.98
/// when they raise no reason): an unlisted cut has no name to share, so it never makes title_ambiguous. The main movie's
/// own label suffix, its dub variants and a Narrative variant belong to its cut and name no second one. Ruling G4: a
/// cut whose chosen title and every sibling fail the 8.2 candidate filter (`kind::candidate_filter(disc, settings)`: a
/// menu loop, a title shorter than `settings.minlength_s` or without a duration) is not planned, with no note and no
/// evidence; `settings.minlength_s` is the only setting read. Ruling H2 (2026-10-19): when such a cut is planned, a
/// main movie whose edition is None because its own label suffix is Other or it carries no label is marked theatrical
/// false, with an evidence sentence, so `edition_names` gives it no "Theatrical Cut" and the owner names both cuts; a
/// main movie the label table names "Theatrical Cut" (the bare `FPL_MainFeature` and `_R` rows) stays theatrical, and
/// without an unlisted cut the Task 5 ruling stands. Def: T5. Use: T5, T7.
pub fn by_database(
    record: &DiscDbRecord,
    matches: &BTreeMap<u32, u32>,
    disc: &Disc,
    wanted: &Wanted,
    settings: &Settings,
) -> Option<MoviePlan> {
    let main_movies = record.main_movies();
    if main_movies.is_empty() {
        return None;
    }
    let media_title = record.media.as_ref().and_then(|media| media.title.as_deref());
    let mut features = Vec::new();
    let mut feature_seconds = Vec::new();
    let mut evidence = Vec::new();
    let mut unnamed = false;
    // H2: the main movies (by position) whose edition is None because they carry no label or their own label suffix is
    // Other; beside an unlisted cut they are left unnamed too.
    let mut nameless = Vec::new();
    for (position, main_movie) in main_movies.iter().enumerate() {
        let number = *matches.get(&main_movie.index)?;
        let title = disc.titles.get(&number)?;
        evidence.push(format!("TheDiscDB's main movie {} is scan title {number}", main_movie.index));
        let item_title = main_movie.item.as_ref().map_or("", |item| item.title.as_str());
        let mut edition = item_edition(item_title, media_title);
        let mut suffix = None;
        if let Some(name) = &edition {
            evidence.push(format!("edition '{name}' from TheDiscDB's item title '{item_title}'"));
        } else if disc.java_in_use {
            if let Some(comment) = title.comment.as_deref() {
                if let Some(label) = parse_label(number, comment) {
                    edition = edition_of(&label.suffix).map(str::to_string);
                    if let Some(name) = &edition {
                        evidence.push(format!("edition '{name}' from the label {comment} of title {number}"));
                    }
                    suffix = Some(label.suffix);
                }
            }
        }
        if edition.is_none() && matches!(suffix, None | Some(Suffix::Other(_))) {
            nameless.push(position);
        }
        let later_unnamed = position > 0 && edition.is_none();
        if later_unnamed {
            unnamed = true;
            evidence.push(format!("scan title {number} is a further main movie with no edition name"));
        }
        let theatrical = !later_unnamed && is_theatrical(edition.as_deref());
        features.push(Feature { title: number, edition, theatrical, siblings: Vec::new() });
        feature_seconds.push(title.seconds);
    }
    // Two main movies given the same edition name would file to one path (Review Focus 8).
    let same_name = same_edition_name(&features);
    if let Some(name) = &same_name {
        evidence.push(format!("two cuts are both named {name}"));
    }
    let mut reviews = Vec::new();
    if unnamed {
        reviews.push(ReviewReason::UnnamedCut);
    }
    if same_name.is_some() {
        reviews.push(ReviewReason::TitleAmbiguous);
    }
    let confidence = if reviews.is_empty() { DATABASE_TITLE_CONFIDENCE } else { AMBIGUOUS_TITLE_CONFIDENCE };
    // OR5: a labelled cut none of whose titles is a main movie's is one TheDiscDB does not list. It joins the plan as an
    // unnamed cut and holds the job for review, but the confidence above stays: it came from the main movies alone.
    let main_titles: BTreeSet<u32> = features.iter().map(|feature| feature.title).collect();
    let main_count = features.len();
    let mut notes = Vec::new();
    if let Some(labelled) = by_labels(disc, wanted) {
        let candidates = candidate_filter(disc, settings);
        for (cut, seconds) in labelled.features.into_iter().zip(labelled.feature_seconds) {
            if main_titles.contains(&cut.title) || cut.siblings.iter().any(|sibling| main_titles.contains(sibling)) {
                continue;
            }
            // G4: a cut none of whose titles passes the 8.2 candidate filter (a menu loop, a title shorter than
            // minlength_s or without a duration) is no feature to rip.
            if !std::iter::once(&cut.title).chain(&cut.siblings).any(|number| candidates.contains(number)) {
                continue;
            }
            let number = cut.title;
            let comment = disc
                .titles
                .get(&number)
                .and_then(|title| title.comment.as_deref())
                .expect("by_labels yields only titles whose comment parsed as a label");
            evidence.push(format!(
                "scan title {number} carries the label {comment}, a cut TheDiscDB does not list as a main movie"
            ));
            notes.push(Noted {
                note: Note::UnlistedLabelCut,
                stream: None,
                detail: format!("title {number}'s label {comment} names a cut TheDiscDB does not list; name it with Assign titles"),
            });
            features.push(Feature { title: number, edition: None, theatrical: false, siblings: cut.siblings });
            feature_seconds.push(seconds);
            reviews.push(ReviewReason::UnnamedCut);
        }
    }
    // H2: beside an unlisted cut, a main movie whose own label names no edition (an Other suffix, or no label) is not
    // the theatrical cut either: the owner names both. The bare `FPL_MainFeature` and `_R` rows named it "Theatrical
    // Cut", so it is not among them.
    if features.len() > main_count {
        for position in nameless {
            let feature = &mut features[position];
            if feature.theatrical {
                feature.theatrical = false;
                evidence.push(format!(
                    "scan title {} is a main movie with no edition name beside a cut TheDiscDB does not list, so it is \
                     left unnamed too",
                    feature.title
                ));
            }
        }
    }
    Some(MoviePlan {
        rule: TitleRule::Database,
        features,
        feature_seconds,
        confidence,
        reviews: sorted_unique(reviews),
        notes,
        rivals: Vec::new(),
        evidence,
    })
}

/// An edition name that names the theatrical or main cut (8.4): none at all, or a name containing the word Theatrical
/// (ignoring ASCII case), such as TheDiscDB's "Theatrical Version". The one test of "theatrical" for every rule and caller.
/// Def: T5. Use: T5, T13.
pub fn is_theatrical(edition: Option<&str>) -> bool {
    edition.map_or(true, |name| find_word_ignoring_ascii_case(name, "theatrical").is_some())
}

/// The first edition name two features would both be filed under (8.4, as `edition_names` names them): "Theatrical Cut"
/// for a theatrical cut, else its edition; an unnamed cut (not theatrical, no edition) is skipped. Rules 1 and 2 lower
/// their confidence when there is one, since two features of one name would file to one path (Review Focus 8).
fn same_edition_name(features: &[Feature]) -> Option<String> {
    let mut named: BTreeSet<&str> = BTreeSet::new();
    features
        .iter()
        .filter_map(|feature| if feature.theatrical { Some(THEATRICAL_CUT) } else { feature.edition.as_deref() })
        .find(|name| !named.insert(*name))
        .map(str::to_string)
}

/// What is trimmed from both ends of an item title once the media title is out: brackets, hyphens, en and em dashes,
/// colons and commas (spaces too).
const EDITION_TRIM: &str = "()[]-:,\u{2013}\u{2014}";

/// The edition an item title names beyond the media title (8.4 "Edition names come from TheDiscDB's title for the item"):
/// the item title with the media title's first whole-word occurrence (ignoring ASCII case) removed, then `EDITION_TRIM`
/// and spaces trimmed from both ends; an item title that does not contain the media title is the edition whole. None when
/// nothing is left, or when there is no media title (the pinned form), since the film's own title cannot be told from an
/// edition there.
fn item_edition(item_title: &str, media_title: Option<&str>) -> Option<String> {
    let media = media_title.map(str::trim).filter(|media| !media.is_empty())?;
    let item = item_title.trim();
    let rest = match find_word_ignoring_ascii_case(item, media) {
        Some(at) => format!("{}{}", &item[..at], &item[at + media.len()..]),
        None => item.to_string(),
    };
    let edition = rest.trim_matches(|c: char| c.is_whitespace() || EDITION_TRIM.contains(c));
    (!edition.is_empty()).then(|| edition.to_string())
}

/// The byte offset of the first occurrence of `needle` in `haystack` that equals it ignoring ASCII case and is neither
/// preceded nor followed by a letter or digit.
fn find_word_ignoring_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    let last = haystack.len().checked_sub(needle.len())?;
    (0..=last).find(|&at| {
        let end = at + needle.len();
        haystack.is_char_boundary(at)
            && haystack.is_char_boundary(end)
            && haystack[at..end].eq_ignore_ascii_case(needle)
            && !haystack[..at].chars().next_back().map_or(false, char::is_alphanumeric)
            && !haystack[end..].chars().next().map_or(false, char::is_alphanumeric)
    })
}

/// Rule 2: only when `disc.java_in_use`; labels grouped by suffix without the language code, one cut per group, in order of
/// each cut's lowest title number; within a cut the variant whose audio covers the most of `wanted`'s languages, then the
/// one without a language suffix, then the lower title number; Narrative variants ignored (the `_Narrative` suffix, and
/// a trailing Narrative token after another suffix, `Label.narrative`, ruling G5). Exactly one cut 0.90; several
/// with recognised suffixes 0.90 as editions, except that two cuts given the same edition name (the `none` and `_R` rows
/// are both "Theatrical Cut") make it 0.60 with title_ambiguous (Review Focus 8); any Other suffix 0.60 with unnamed_cut.
/// None when no label is found. When Java was not reported, returns None and plan::plan adds the note
/// LabelRuleUnavailable. Def: T5. Use: T5, T7.
pub fn by_labels(disc: &Disc, wanted: &Wanted) -> Option<MoviePlan> {
    if !disc.java_in_use {
        return None;
    }
    let labels: Vec<(Label, &Title)> = disc
        .titles
        .values()
        .filter_map(|title| parse_label(title.number, title.comment.as_deref()?).map(|label| (label, title)))
        .filter(|(label, _)| label.suffix != Suffix::Narrative && !label.narrative)
        .collect();
    if labels.is_empty() {
        return None;
    }
    // The labels are in title order, so a cut's first label is its lowest title number.
    let mut cuts: Vec<(Suffix, Vec<usize>)> = Vec::new();
    for (index, (label, _)) in labels.iter().enumerate() {
        let cut = cut_of(&label.suffix);
        match cuts.iter_mut().find(|(key, _)| *key == cut) {
            Some((_, members)) => members.push(index),
            None => cuts.push((cut, vec![index])),
        }
    }
    let mut features = Vec::new();
    let mut feature_seconds = Vec::new();
    let mut evidence = Vec::new();
    let mut unnamed = false;
    for (cut, members) in &cuts {
        let chosen = *members.iter().max_by_key(|&&index| {
            let (label, title) = &labels[index];
            (coverage(title, wanted), label.language.is_none(), std::cmp::Reverse(label.title))
        })?;
        let (label, title) = &labels[chosen];
        let titles: Vec<u32> = members.iter().map(|&index| labels[index].0.title).collect();
        evidence.push(format!("label cut {}: titles {titles:?}, title {} chosen", label_text(cut), label.title));
        let edition = edition_of(cut).map(str::to_string);
        let is_unnamed = matches!(cut, Suffix::Other(_));
        unnamed |= is_unnamed;
        let theatrical = !is_unnamed && is_theatrical(edition.as_deref());
        let siblings = titles.iter().copied().filter(|&number| number != label.title).collect();
        features.push(Feature { title: label.title, edition, theatrical, siblings });
        feature_seconds.push(title.seconds);
    }
    // Two cuts given the same edition name (the table's `none` and `_R` rows) would file to one path (Review Focus 8).
    let same_name = same_edition_name(&features);
    let mut reviews = Vec::new();
    if unnamed {
        reviews.push(ReviewReason::UnnamedCut);
    }
    if let Some(name) = &same_name {
        reviews.push(ReviewReason::TitleAmbiguous);
        evidence.push(format!("two cuts are both named {name}"));
    }
    let confidence = if reviews.is_empty() { LABELS_TITLE_CONFIDENCE } else { AMBIGUOUS_TITLE_CONFIDENCE };
    Some(MoviePlan {
        rule: TitleRule::Labels,
        features,
        feature_seconds,
        confidence,
        reviews: sorted_unique(reviews),
        notes: Vec::new(),
        rivals: Vec::new(),
        evidence,
    })
}

/// The cut a suffix belongs to: a dub variant (a suffix that is only a language code) belongs to the cut without a suffix.
fn cut_of(suffix: &Suffix) -> Suffix {
    match suffix {
        Suffix::Language(_) => Suffix::None,
        other => other.clone(),
    }
}

/// A cut's label as a disc writes it, for the evidence.
fn label_text(cut: &Suffix) -> String {
    match cut {
        Suffix::None => LABEL_PREFIX.to_string(),
        Suffix::R => format!("{LABEL_PREFIX}_R"),
        Suffix::Ur => format!("{LABEL_PREFIX}_UR"),
        Suffix::Ext => format!("{LABEL_PREFIX}_EXT"),
        Suffix::Narrative => format!("{LABEL_PREFIX}_Narrative"),
        Suffix::Language(code) => format!("{LABEL_PREFIX}_{}", code.to_ascii_uppercase()),
        Suffix::Other(text) => format!("{LABEL_PREFIX}_{text}"),
    }
}

/// How many of the wanted languages a title's audio covers: English when an audio stream carries an English code
/// (`lang::is_english`, "eng" or "en" in any case, SR4), and the original language when it is not English and an audio
/// stream carries one of its codes (8.3 rule 2, 8.6 "Languages").
fn coverage(title: &Title, wanted: &Wanted) -> usize {
    let audio = title.streams_of(StreamKind::Audio);
    let languages: Vec<&str> = audio.iter().filter_map(|stream| stream.language.as_deref()).collect();
    let english = languages.iter().any(|code| is_english(code));
    let original = !wanted.original_is_english && languages.iter().any(|code| wanted.is_original(code));
    usize::from(english) + usize::from(original)
}

/// What rule 3 found. Def: T5. Use: T7.
#[derive(Clone, Debug, PartialEq)]
pub enum StructureOutcome {
    /// Rule 3.1 under OR1: the group at this index of the groups is a "play all" title of episode-length parts, so the disc
    /// is TV (8.5).
    PlayAll { group: usize },
    Plan(MoviePlan),
}

/// Rule 3 on the groups: 3.1 under OR1 (a "play all" title of episode-length parts, `kind::play_all_of_episodes`; the disc
/// is TV); then `by_structure_as_movie`'s 3.2 to 3.7. A play-all of shorter parts, such as an extras playlist, is an
/// ordinary group here. Reads `settings.feature_min_minutes` only. Def: T5. Use: T5, T7.
pub fn by_structure(disc: &Disc, groups: &[Group], settings: &Settings) -> StructureOutcome {
    if let Some(group) = play_all_of_episodes(groups, disc.is_dvd) {
        return StructureOutcome::PlayAll { group };
    }
    StructureOutcome::Plan(by_structure_as_movie(disc, groups, settings))
}

/// Rule 3 without 3.1, for a disc that is a movie whatever its groups' shape (8.2 "TheDiscDB states the kind when it
/// knows the disc", so a play-all-shaped group is an ordinary group here): 3.2 keep groups >= settings.feature_min_minutes,
/// else the longest; 3.3 rivals within RIVAL_BAND of the longest kept; 3.4 in each rival the title with chapters, then the
/// one whose audio and subtitle streams cover every other's, counted per signature (kind, language, codec family,
/// channels: it holds at least as many streams of each signature as every other title), else 0.60 title_ambiguous; 3.5
/// one rival 0.85; 3.6 every pair's segment_difference <= 0.25 gives 0.60 title_ambiguous; 3.7 else 0.60
/// multiple_features. Reads `settings.feature_min_minutes` only. No group at all (no candidate) gives a plan with no
/// feature, 0.60 and title_ambiguous. Def: T5. Use: T5, T7.
pub fn by_structure_as_movie(disc: &Disc, groups: &[Group], settings: &Settings) -> MoviePlan {
    if groups.is_empty() {
        return MoviePlan {
            rule: TitleRule::Structure,
            features: Vec::new(),
            feature_seconds: Vec::new(),
            confidence: AMBIGUOUS_TITLE_CONFIDENCE,
            reviews: vec![ReviewReason::TitleAmbiguous],
            notes: Vec::new(),
            rivals: Vec::new(),
            evidence: vec!["no title passed the candidate filter".to_string()],
        };
    }
    let mut evidence = Vec::new();
    let minimum = u64::from(settings.feature_min_minutes) * 60;
    let long: Vec<&Group> = groups.iter().filter(|group| u64::from(group.seconds) >= minimum).collect();
    let kept: Vec<&Group> = if long.is_empty() {
        let longest = longest_group(groups);
        evidence.push(format!(
            "no group is at least {} minutes long; the longest, {}, is kept",
            settings.feature_min_minutes, longest.key
        ));
        vec![longest]
    } else {
        evidence.push(format!(
            "groups at least {} minutes long: {} of {}",
            settings.feature_min_minutes,
            long.len(),
            groups.len()
        ));
        long
    };
    let longest = kept.iter().map(|group| group.seconds).max().unwrap_or(0);
    let rivals: Vec<&Group> = kept
        .into_iter()
        .filter(|group| within(f64::from(group.seconds), f64::from(longest), RIVAL_BAND))
        .collect();
    let mut features = Vec::new();
    let mut feature_seconds = Vec::new();
    let mut reviews = Vec::new();
    for group in &rivals {
        let (chosen, how, clear) = choose_title(disc, group);
        if !clear {
            reviews.push(ReviewReason::TitleAmbiguous);
        }
        evidence.push(format!("rival group {}: title {chosen} ({how})", group.key));
        let siblings = group.titles.iter().copied().filter(|&number| number != chosen).collect();
        features.push(Feature { title: chosen, edition: None, theatrical: true, siblings });
        feature_seconds.push(disc.titles.get(&chosen).and_then(|title| title.seconds));
    }
    if rivals.len() >= 2 {
        let mut largest: f64 = 0.0;
        for (index, first) in rivals.iter().enumerate() {
            for second in &rivals[index + 1..] {
                largest = largest.max(segment_difference(&first.segments, &second.segments));
            }
        }
        evidence.push(format!(
            "{} rivals; the largest segment difference between two of them is {largest:.2}",
            rivals.len()
        ));
        reviews.push(if largest <= SEGMENT_DIFFERENCE_MAX {
            ReviewReason::TitleAmbiguous
        } else {
            ReviewReason::MultipleFeatures
        });
    }
    let confidence = if rivals.len() == 1 && reviews.is_empty() {
        STRUCTURE_TITLE_CONFIDENCE
    } else {
        AMBIGUOUS_TITLE_CONFIDENCE
    };
    MoviePlan {
        rule: TitleRule::Structure,
        features,
        feature_seconds,
        confidence,
        reviews: sorted_unique(reviews),
        notes: Vec::new(),
        rivals: rivals.iter().map(|group| group.key.clone()).collect(),
        evidence,
    }
}

/// The first group with the longest duration (`groups` is not empty).
fn longest_group(groups: &[Group]) -> &Group {
    let mut longest = &groups[0];
    for group in &groups[1..] {
        if group.seconds > longest.seconds {
            longest = group;
        }
    }
    longest
}

/// Rule 3.4 within one rival group: (the title chosen, how, whether the choice is clear). A group of one title is clear;
/// otherwise the titles with chapters stay in the running when any has chapters (all of them when none has), one left is
/// chosen, and among several the first whose streams cover every other remaining title's (`Title::signature_counts`: at
/// least as many streams of each signature, so a title with one more stream of a signature covers the other) is chosen;
/// when none is, the first remaining title is kept and the choice is not clear.
fn choose_title(disc: &Disc, group: &Group) -> (u32, &'static str, bool) {
    let members: Vec<&Title> = group.titles.iter().filter_map(|number| disc.titles.get(number)).collect();
    if members.len() <= 1 {
        return (group.titles[0], "its only title", true);
    }
    let with_chapters: Vec<&Title> = members.iter().copied().filter(|title| title.chapters > 0).collect();
    let remaining = if with_chapters.is_empty() { members } else { with_chapters };
    if remaining.len() == 1 {
        return (remaining[0].number, "the title with chapters", true);
    }
    let signatures: Vec<BTreeMap<_, u32>> = remaining.iter().map(|title| title.signature_counts()).collect();
    let covers = |wide: &BTreeMap<_, u32>, narrow: &BTreeMap<_, u32>| {
        narrow.iter().all(|(signature, count)| wide.get(signature).is_some_and(|have| have >= count))
    };
    let covering = (0..remaining.len()).find(|&index| {
        (0..remaining.len()).all(|other| other == index || covers(&signatures[index], &signatures[other]))
    });
    match covering {
        Some(index) => (remaining[index].number, "its streams cover every other title's", true),
        None => (remaining[0].number, "no title's streams cover every other's", false),
    }
}

/// Segments in exactly one of the two lists, divided by the longer list's length; 0.0 for two empty lists. Spec 8.3 rule 3.6. Def: T5. Use: T5.
pub fn segment_difference(a: &[i64], b: &[i64]) -> f64 {
    let longer = a.len().max(b.len());
    if longer == 0 {
        return 0.0;
    }
    let left: BTreeSet<i64> = a.iter().copied().collect();
    let right: BTreeSet<i64> = b.iter().copied().collect();
    left.symmetric_difference(&right).count() as f64 / longer as f64
}

/// Rule 4: a plan from rule 3 whose every feature's duration (`plan.feature_seconds`) is more than 10% from
/// `runtime_min * 60` drops to 0.60 with title_ambiguous (Open point 35); rule 1 and 2 plans and None runtimes are
/// returned unchanged. A runtime of 0 minutes counts as none, and a feature without a duration is never more than 10% off,
/// so a plan with one keeps its confidence. Def: T5. Use: T7.
pub fn runtime_sanity(mut plan: MoviePlan, runtime_min: Option<u32>) -> MoviePlan {
    let runtime = match runtime_min {
        Some(runtime) if runtime > 0 => runtime,
        _ => return plan,
    };
    if plan.rule != TitleRule::Structure || plan.feature_seconds.is_empty() {
        return plan;
    }
    let target = f64::from(runtime) * 60.0;
    let every_feature_off = plan
        .feature_seconds
        .iter()
        .all(|seconds| seconds.map_or(false, |seconds| !within(f64::from(seconds), target, RUNTIME_SANITY_BAND)));
    if !every_feature_off {
        return plan;
    }
    plan.confidence = plan.confidence.min(AMBIGUOUS_TITLE_CONFIDENCE);
    plan.reviews.push(ReviewReason::TitleAmbiguous);
    plan.reviews = sorted_unique(plan.reviews);
    plan.evidence.push(format!("no feature is within 10% of the TMDb runtime of {runtime} minutes"));
    plan
}

/// First applicable rule (8.3): by_database, else by_labels, else by_structure. `wanted` reaches rule 1 too, which uses it
/// to choose the variant of each labelled cut TheDiscDB does not list (OR5, SR3), and so does `settings`, whose candidate
/// filter (8.2) leaves out such a cut when none of its titles passes it (G4). Def: T5. Use: T7.
pub fn movie_plan(disc: &Disc, discdb: Option<&DiscDbRecord>, matches: &BTreeMap<u32, u32>, groups: &[Group], wanted: &Wanted, settings: &Settings) -> StructureOutcome {
    if let Some(plan) = discdb.and_then(|record| by_database(record, matches, disc, wanted, settings)) {
        return StructureOutcome::Plan(plan);
    }
    if let Some(plan) = by_labels(disc, wanted) {
        return StructureOutcome::Plan(plan);
    }
    by_structure(disc, groups, settings)
}

/// What phase 2 reads from the library for 8.4 (RP18). Def: T5. Use: T7, T11, T13, T15.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LibraryFacts {
    /// "the film's library folder already holds an edition-named file".
    pub folder_has_edition_named_file: bool,
    /// Earlier filed copies of this film: cut key ("theatrical", or the lower-cased edition name) to the edition name used
    /// ("" when that copy was filed without an edition name; this cut then gets None).
    pub earlier_edition_names: BTreeMap<String, String>,
    /// Spec 7: the final path already exists and is not this job's file. Phase 2 checks the plain path (no copy label);
    /// under the owner's Resolve collision "keep both" it must also check the copy-labelled path (`- 2160p 2`) before
    /// filing and hold the job with path_collision when that one is taken too (Open point 39).
    pub path_collision: bool,
}

/// The cut key of the theatrical or main cut in `LibraryFacts.earlier_edition_names`.
const THEATRICAL_KEY: &str = "theatrical";

/// 8.4's three rules, per feature in order: a non-theatrical cut always carries its name, even when it is the only cut; the
/// theatrical cut carries "Theatrical Cut" only when `features.len() >= 2` or `facts.folder_has_edition_named_file`; an
/// earlier filed copy's name (`earlier_edition_names`) replaces either. Names are raw here; naming::edition_label sanitises
/// and caps them. An earlier name "" gives None; an unnamed cut (not theatrical, no edition) gives None until review names
/// it. Def: T5. Use: T7, T13.
pub fn edition_names(features: &[Feature], facts: &LibraryFacts) -> Vec<Option<String>> {
    features
        .iter()
        .map(|feature| {
            let key = if feature.theatrical {
                Some(THEATRICAL_KEY.to_string())
            } else {
                feature.edition.as_ref().map(|name| name.to_lowercase())
            };
            if let Some(name) = key.as_ref().and_then(|key| facts.earlier_edition_names.get(key)) {
                return (!name.is_empty()).then(|| name.clone());
            }
            if !feature.theatrical {
                return feature.edition.clone();
            }
            (features.len() >= 2 || facts.folder_has_edition_named_file).then(|| THEATRICAL_CUT.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::wanted_languages;
    use crate::lookup::discdb::match_titles;
    use crate::plan::fixtures::*;
    use crate::plan::kind::{candidate_filter, groups};
    use crate::testsupport::{scan_text, scan_text_with_messages};
    use serde_json::json;

    /// `parse_label` of a comment on title 7.
    fn label(comment: &str) -> Option<Label> {
        parse_label(7, comment)
    }

    /// The label `label` should give, with no trailing Narrative token.
    fn expected(suffix: Suffix, language: Option<&str>) -> Option<Label> {
        Some(Label { title: 7, suffix, language: language.map(str::to_string), narrative: false })
    }

    #[test]
    fn a_label_without_a_suffix_is_the_feature() {
        assert_eq!(label("FPL_MainFeature"), expected(Suffix::None, None));
        assert_eq!(edition_of(&Suffix::None), Some("Theatrical Cut"));
    }

    #[test]
    fn the_r_suffix_is_the_theatrical_cut() {
        assert_eq!(label("FPL_MainFeature_R"), expected(Suffix::R, None));
        assert_eq!(label("FPL_MainFeature_r"), expected(Suffix::R, None));
        assert_eq!(edition_of(&Suffix::R), Some("Theatrical Cut"));
    }

    #[test]
    fn the_ur_suffix_is_the_unrated_cut() {
        assert_eq!(label("FPL_MainFeature_UR"), expected(Suffix::Ur, None));
        assert_eq!(edition_of(&Suffix::Ur), Some("Unrated Cut"));
    }

    #[test]
    fn the_ext_suffix_is_the_extended_cut() {
        assert_eq!(label("FPL_MainFeature_EXT"), expected(Suffix::Ext, None));
        assert_eq!(edition_of(&Suffix::Ext), Some("Extended Cut"));
    }

    #[test]
    fn the_narrative_suffix_is_an_audio_description_variant_with_no_edition() {
        assert_eq!(label("FPL_MainFeature_Narrative"), expected(Suffix::Narrative, None));
        assert_eq!(edition_of(&Suffix::Narrative), None);
    }

    #[test]
    fn a_trailing_narrative_token_is_the_audio_description_variant_of_its_cut() {
        // G5: `_UR_Narrative` is the UR cut's audio-description variant, read as the language code is split off.
        assert_eq!(label("FPL_MainFeature_UR_Narrative"), Some(Label { title: 7, suffix: Suffix::Ur, language: None, narrative: true }));
        assert_eq!(label("FPL_MainFeature_ur_NARRATIVE"), Some(Label { title: 7, suffix: Suffix::Ur, language: None, narrative: true }));
        // The language code is split off first, so it may follow the Narrative token.
        assert_eq!(
            label("FPL_MainFeature_UR_Narrative_JPN"),
            Some(Label { title: 7, suffix: Suffix::Ur, language: Some("jpn".to_string()), narrative: true })
        );
        // The bare suffix keeps its own row, and so does its dub variant.
        assert_eq!(label("FPL_MainFeature_Narrative"), expected(Suffix::Narrative, None));
        assert_eq!(label("FPL_MainFeature_Narrative_JPN"), expected(Suffix::Narrative, Some("jpn")));
    }

    #[test]
    fn a_trailing_language_code_is_a_dub_variant_of_the_same_cut() {
        assert_eq!(label("FPL_MainFeature_JPN"), expected(Suffix::Language("jpn".to_string()), Some("jpn")));
        assert_eq!(label("FPL_MainFeature_ENG"), expected(Suffix::Language("eng".to_string()), Some("eng")));
        assert_eq!(label("FPL_MainFeature_UR_JPN"), expected(Suffix::Ur, Some("jpn")));
        assert_eq!(edition_of(&Suffix::Language("jpn".to_string())), None);
    }

    #[test]
    fn any_other_suffix_is_an_unnamed_cut() {
        assert_eq!(label("FPL_MainFeature_DC"), expected(Suffix::Other("DC".to_string()), None));
        assert_eq!(label("FPL_MainFeature_UR2"), expected(Suffix::Other("UR2".to_string()), None));
        assert_eq!(edition_of(&Suffix::Other("DC".to_string())), None);
    }

    #[test]
    fn a_label_may_be_preceded_by_seg() {
        assert_eq!(label("SEG_FPL_MainFeature_UR"), expected(Suffix::Ur, None));
        assert_eq!(label("SEG FPL_MainFeature"), expected(Suffix::None, None));
    }

    #[test]
    fn other_comments_are_not_labels() {
        for comment in ["SEG FPL_02", "TMPL Main Menu", "SPBD Copy Disclaimer Playlist", "B1", "SF_Extended", ""] {
            assert_eq!(label(comment), None, "{comment:?}");
        }
    }

    #[test]
    fn three_letters_that_are_not_a_language_code_are_not_split_off() {
        assert_eq!(label("FPL_MainFeature_ABC"), expected(Suffix::Other("ABC".to_string()), None));
        assert_eq!(label("FPL_MainFeature_UR_ABC"), expected(Suffix::Other("UR_ABC".to_string()), None));
    }

    #[test]
    fn the_pinned_labels_parse_as_the_table_reads_them() {
        let labels_of = |disc: &Disc| -> Vec<Label> {
            disc.titles.values().filter_map(|title| parse_label(title.number, title.comment.as_deref()?)).collect()
        };
        assert_eq!(
            labels_of(&pinned(EXPENDABLES)),
            vec![
                Label { title: 41, suffix: Suffix::Ur, language: None, narrative: false },
                Label { title: 54, suffix: Suffix::R, language: None, narrative: false },
            ]
        );
        assert_eq!(labels_of(&pinned(SMILE)), vec![Label { title: 14, suffix: Suffix::None, language: None, narrative: false }]);
    }

    /// One feature as the rules build it.
    fn feature(title: u32, edition: Option<&str>, theatrical: bool, siblings: &[u32]) -> Feature {
        Feature { title, edition: edition.map(str::to_string), theatrical, siblings: siblings.to_vec() }
    }

    /// Owned strings, for evidence and rival keys.
    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    /// A TheDiscDB answer in the API's shape with one MainMovie per item title: database index i is `0000<i+1>.mpls`
    /// with segment map `<i+1>`; `media.title` is `media_title` when given.
    fn record(media_title: Option<&str>, items: &[&str]) -> DiscDbRecord {
        let titles: Vec<serde_json::Value> = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                json!({
                    "index": index,
                    "source_file": format!("{:05}.mpls", index + 1),
                    "segment_map": (index + 1).to_string(),
                    "item": {"title": item, "type": "MainMovie"}
                })
            })
            .collect();
        let mut value = json!({ "titles": titles });
        if let Some(title) = media_title {
            value["media"] = json!({"title": title, "type": "Movie"});
        }
        serde_json::from_value(value).expect("the synthetic record parses")
    }

    /// A Blu-ray scan of two feature-length titles: 0 (`00001.mpls`, segment map 1, 100 minutes, comment `comment`) and 1
    /// (`00002.mpls`, segment map 2, 110 minutes), with Java in use or not.
    fn two_titles(java: bool, comment: &str) -> Disc {
        disc_of(&scan_text_with_messages(
            "1.18.2",
            java,
            BLURAY,
            &[
                (0, &[(9, "1:40:00"), (8, "16"), (16, "00001.mpls"), (26, "1"), (49, comment)], &[(0, VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "1:50:00"), (8, "16"), (16, "00002.mpls"), (26, "2")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ))
    }

    /// Rule 1 over a pinned log and its TheDiscDB answer, matched by `match_titles`.
    fn database(log: &str, hash: &str) -> Option<MoviePlan> {
        let disc = pinned(log);
        let record = api_record(hash);
        by_database(&record, &match_titles(&record, &disc), &disc, &wanted_languages(None), &Settings::default())
    }

    #[test]
    fn expendables_main_movie_takes_its_edition_from_the_label_table_and_its_r_label_is_an_unlisted_unnamed_cut() {
        // OR5: TheDiscDB lists title 41 only; title 54 carries FPL_MainFeature_R, a cut it does not list.
        assert_eq!(
            database(EXPENDABLES, EXPENDABLES_API),
            Some(MoviePlan {
                rule: TitleRule::Database,
                features: vec![feature(41, Some("Unrated Cut"), false, &[]), feature(54, None, false, &[])],
                feature_seconds: vec![Some(7891), Some(7587)],
                confidence: 0.98,
                reviews: vec![ReviewReason::UnnamedCut],
                notes: vec![Noted {
                    note: Note::UnlistedLabelCut,
                    stream: None,
                    detail: "title 54's label FPL_MainFeature_R names a cut TheDiscDB does not list; name it with Assign titles"
                        .to_string(),
                }],
                rivals: vec![],
                evidence: strings(&[
                    "TheDiscDB's main movie 41 is scan title 41",
                    "edition 'Unrated Cut' from the label FPL_MainFeature_UR of title 41",
                    "scan title 54 carries the label FPL_MainFeature_R, a cut TheDiscDB does not list as a main movie",
                ]),
            })
        );
    }

    #[test]
    fn john_wick_uhd_main_movie_is_the_playlist_not_its_clip() {
        let plan = database(JOHN_WICK_UHD, JOHN_WICK_UHD_API).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(0, None, true, &[])]);
        assert_eq!(plan.feature_seconds, vec![Some(6073)]);
        assert_eq!((plan.confidence, plan.reviews), (DATABASE_TITLE_CONFIDENCE, vec![]));
        assert_eq!(plan.evidence, strings(&["TheDiscDB's main movie 0 is scan title 0"]));
    }

    #[test]
    fn knowing_main_movie_is_matched_on_a_dvd_by_its_title_number() {
        let plan = database(KNOWING, KNOWING_API).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(0, None, true, &[])]);
        assert_eq!(plan.confidence, 0.98);
    }

    #[test]
    fn smile_main_movie_takes_the_theatrical_label() {
        let plan = database(SMILE, SMILE_API).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(14, Some("Theatrical Cut"), true, &[])]);
        assert_eq!(plan.feature_seconds, vec![Some(6937)]);
        assert_eq!(plan.confidence, 0.98);
    }

    #[test]
    fn a_record_without_a_main_movie_gives_no_database_plan() {
        assert_eq!(database(CHERNOBYL_1, CHERNOBYL_1_API), None);
    }

    #[test]
    fn a_main_movie_that_matches_no_scan_title_gives_no_database_plan() {
        let disc = pinned(JOHN_WICK_UHD);
        assert_eq!(by_database(&api_record(JOHN_WICK_UHD_API), &BTreeMap::new(), &disc, &wanted_languages(None), &Settings::default()), None);
        // Every main movie must reconcile: one of two matching is not enough.
        let two = record(Some("Film"), &["Film", "Film (Director's Cut)"]);
        assert_eq!(by_database(&two, &BTreeMap::from([(0, 0)]), &two_titles(false, "B1"), &wanted_languages(None), &Settings::default()), None);
    }

    #[test]
    fn the_edition_text_drops_the_media_title_brackets_and_dashes() {
        assert_eq!(item_edition("The Expendables 3", Some("The Expendables 3")), None);
        assert_eq!(item_edition("Film (Director's Cut)", Some("Film")), Some("Director's Cut".to_string()));
        assert_eq!(item_edition("Film - Extended Edition", Some("Film")), Some("Extended Edition".to_string()));
        assert_eq!(item_edition("FILM: Final Cut", Some("Film")), Some("Final Cut".to_string()));
        assert_eq!(item_edition("Film [Unrated]", Some("Film")), Some("Unrated".to_string()));
        assert_eq!(item_edition("Director's Cut", Some("Film")), Some("Director's Cut".to_string()));
        assert_eq!(item_edition("Filmmaker's Cut", Some("Film")), Some("Filmmaker's Cut".to_string()));
        assert_eq!(item_edition("Knowing", None), None);
        assert_eq!(item_edition("Knowing", Some("  ")), None);
    }

    #[test]
    fn an_edition_is_the_item_title_beyond_the_media_title() {
        let two = record(Some("Film"), &["Film", "Film (Director's Cut)"]);
        let plan = by_database(&two, &BTreeMap::from([(0, 0), (1, 1)]), &two_titles(false, "B1"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(0, None, true, &[]), feature(1, Some("Director's Cut"), false, &[])]);
        assert_eq!(plan.feature_seconds, vec![Some(6000), Some(6600)]);
        assert_eq!((plan.confidence, plan.reviews), (0.98, vec![]));
        assert_eq!(
            plan.evidence,
            strings(&[
                "TheDiscDB's main movie 0 is scan title 0",
                "TheDiscDB's main movie 1 is scan title 1",
                "edition 'Director's Cut' from TheDiscDB's item title 'Film (Director's Cut)'",
            ])
        );
    }

    #[test]
    fn a_later_main_movie_with_no_edition_is_an_unnamed_cut() {
        let two = record(Some("Film"), &["Film", "Film"]);
        let plan = by_database(&two, &BTreeMap::from([(0, 0), (1, 1)]), &two_titles(false, "B1"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(0, None, true, &[]), feature(1, None, false, &[])]);
        assert_eq!((plan.confidence, plan.reviews), (AMBIGUOUS_TITLE_CONFIDENCE, vec![ReviewReason::UnnamedCut]));
        assert_eq!(plan.evidence.last().map(String::as_str), Some("scan title 1 is a further main movie with no edition name"));
    }

    #[test]
    fn two_main_movies_with_the_same_edition_name_are_ambiguous() {
        // Both are the theatrical cut, so edition_names would file both as "Theatrical Cut" (Review Focus 8).
        let two = record(Some("Film"), &["Film", "Film (Theatrical Cut)"]);
        let plan = by_database(&two, &BTreeMap::from([(0, 0), (1, 1)]), &two_titles(false, "B1"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(0, None, true, &[]), feature(1, Some("Theatrical Cut"), true, &[])]);
        assert_eq!((plan.confidence, plan.reviews), (0.60, vec![ReviewReason::TitleAmbiguous]));
        assert_eq!(plan.evidence.last().map(String::as_str), Some("two cuts are both named Theatrical Cut"));
    }

    #[test]
    fn the_label_table_names_a_main_movie_only_when_java_is_in_use() {
        let one = record(Some("Film"), &["Film"]);
        let matches = BTreeMap::from([(0, 0)]);
        let without_java = by_database(&one, &matches, &two_titles(false, "FPL_MainFeature_UR"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(without_java.features, vec![feature(0, None, true, &[])]);
        let with_java = by_database(&one, &matches, &two_titles(true, "FPL_MainFeature_UR"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(with_java.features, vec![feature(0, Some("Unrated Cut"), false, &[])]);
        // The pinned form has no media title, so its item titles name no edition.
        let pinned_form = record(None, &["Film (Director's Cut)"]);
        let plan = by_database(&pinned_form, &matches, &two_titles(false, "B1"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(0, None, true, &[])]);
    }

    #[test]
    fn a_java_label_naming_a_cut_thediscdb_does_not_list_is_an_unnamed_cut_beside_the_main_movie_at_0_98() {
        // OR5: rule 1 still wins, and the labelled cut TheDiscDB does not list is ripped as an unnamed cut for Assign titles.
        let one = record(Some("Film"), &["Film"]);
        let main_is_title_1 = BTreeMap::from([(0, 1)]);
        let english = wanted_languages(None);
        let disc = java_disc(&[(1, "FPL_MainFeature_UR", &[ENG_AC3]), (2, "FPL_MainFeature_R", &[ENG_AC3])]);
        let plan = by_database(&one, &main_is_title_1, &disc, &english, &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.rule, TitleRule::Database);
        assert_eq!(plan.features, vec![feature(1, Some("Unrated Cut"), false, &[]), feature(2, None, false, &[])]);
        assert_eq!(plan.feature_seconds, vec![Some(6000), Some(6000)]);
        assert_eq!((plan.confidence, plan.reviews.clone()), (0.98, vec![ReviewReason::UnnamedCut]));
        assert_eq!(
            plan.notes,
            vec![Noted {
                note: Note::UnlistedLabelCut,
                stream: None,
                detail: "title 2's label FPL_MainFeature_R names a cut TheDiscDB does not list; name it with Assign titles".to_string(),
            }]
        );
        assert_eq!(
            plan.evidence.last().map(String::as_str),
            Some("scan title 2 carries the label FPL_MainFeature_R, a cut TheDiscDB does not list as a main movie")
        );
        // OR5: a cut present only as a dub variant is unlisted too, its chosen title the variant.
        let dub_only = java_disc(&[(1, "FPL_MainFeature_UR", &[ENG_AC3]), (2, "FPL_MainFeature_R_JPN", &[JPN_AC3])]);
        let plan = by_database(&one, &main_is_title_1, &dub_only, &english, &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(1, Some("Unrated Cut"), false, &[]), feature(2, None, false, &[])]);
        // The Task 5 ruling stands: the main movie's own suffix, its dub variant and a Narrative variant name no second cut.
        let own = java_disc(&[
            (1, "FPL_MainFeature_DC", &[ENG_AC3]),
            (2, "FPL_MainFeature_DC_JPN", &[JPN_AC3]),
            (3, "FPL_MainFeature_Narrative", &[ENG_AC3]),
        ]);
        let plan = by_database(&one, &main_is_title_1, &own, &english, &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(1, None, true, &[])]);
        assert_eq!((plan.confidence, plan.reviews, plan.notes), (0.98, vec![], vec![]));
        // G5: a Narrative token after the main movie's own suffix marks its audio-description variant, no second cut.
        let narrated = java_disc(&[(1, "FPL_MainFeature_UR", &[ENG_AC3]), (3, "FPL_MainFeature_UR_Narrative", &[ENG_AC3])]);
        let plan = by_database(&one, &main_is_title_1, &narrated, &english, &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(1, Some("Unrated Cut"), false, &[])]);
        assert_eq!((plan.confidence, plan.reviews, plan.notes), (0.98, vec![], vec![]));
        // Labels are trusted only when Java is in use (9.4): without MSG:3344 the labelled title 0 is not a second cut.
        let plain = by_database(&one, &main_is_title_1, &two_titles(false, "FPL_MainFeature_R"), &english, &Settings::default()).expect("rule 1 applies");
        assert_eq!((plain.features, plain.reviews), (vec![feature(1, None, true, &[])], vec![]));
        // An unlisted cut never makes title_ambiguous: it has no name to share.
        let unnamed_main = record(Some("Film"), &["Film"]);
        let plan = by_database(&unnamed_main, &main_is_title_1, &java_disc(&[(1, "FPL_MainFeature", &[ENG_AC3]), (2, "FPL_MainFeature_R", &[ENG_AC3])]), &english, &Settings::default()).expect("rule 1 applies");
        assert_eq!((plan.confidence, plan.reviews), (0.98, vec![ReviewReason::UnnamedCut]));
    }

    #[test]
    fn beside_an_unlisted_cut_a_main_movie_whose_label_names_no_edition_is_left_unnamed_too() {
        // H2: the main movie's own label suffix is Other (`_DC`) or it carries no label, so it has no edition; beside a
        // cut TheDiscDB does not list it is no longer the theatrical cut, and the owner names both cuts.
        let one = record(Some("Film"), &["Film"]);
        let main_is_title_1 = BTreeMap::from([(0, 1)]);
        let english = wanted_languages(None);
        let other = java_disc(&[(1, "FPL_MainFeature_DC", &[ENG_AC3]), (2, "FPL_MainFeature", &[ENG_AC3])]);
        let unlabelled = java_disc(&[(1, "", &[ENG_AC3]), (2, "FPL_MainFeature_R", &[ENG_AC3])]);
        for disc in [&other, &unlabelled] {
            let plan = by_database(&one, &main_is_title_1, disc, &english, &Settings::default()).expect("rule 1 applies");
            assert_eq!(plan.features, vec![feature(1, None, false, &[]), feature(2, None, false, &[])]);
            assert_eq!((plan.confidence, plan.reviews.clone()), (0.98, vec![ReviewReason::UnnamedCut]));
            assert_eq!(plan.notes.iter().map(|noted| noted.note).collect::<Vec<Note>>(), vec![Note::UnlistedLabelCut]);
            let unnamed = "scan title 1 is a main movie with no edition name beside a cut TheDiscDB does not list, so it is \
                           left unnamed too";
            assert!(plan.evidence.contains(&unnamed.to_string()), "{:?}", plan.evidence);
            assert_eq!(edition_names(&plan.features, &LibraryFacts::default()), names(&[None, None]));
        }
        // A main movie whose label is the bare `FPL_MainFeature` or `_R` row is named "Theatrical Cut" by the label table
        // and stays the theatrical cut beside an unlisted one.
        for main_label in ["FPL_MainFeature", "FPL_MainFeature_R"] {
            let disc = java_disc(&[(1, main_label, &[ENG_AC3]), (2, "FPL_MainFeature_UR", &[ENG_AC3])]);
            let plan = by_database(&one, &main_is_title_1, &disc, &english, &Settings::default()).expect("rule 1 applies");
            assert_eq!(
                plan.features,
                vec![feature(1, Some("Theatrical Cut"), true, &[]), feature(2, None, false, &[])],
                "{main_label}"
            );
            assert_eq!(
                edition_names(&plan.features, &LibraryFacts::default()),
                names(&[Some("Theatrical Cut"), None]),
                "{main_label}"
            );
        }
        // Without an unlisted cut the Task 5 ruling stands: the main movie with an Other suffix stays theatrical.
        let alone = java_disc(&[(1, "FPL_MainFeature_DC", &[ENG_AC3])]);
        let plan = by_database(&one, &main_is_title_1, &alone, &english, &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(1, None, true, &[])]);
    }

    #[test]
    fn a_labelled_cut_the_candidate_filter_excludes_is_not_planned_beside_the_main_movie() {
        // G4: title 2 carries FPL_MainFeature_R but has no audio stream, a menu loop the 8.2 candidate filter leaves
        // out, so rule 1 plans the main movie alone, with no unnamed cut, no review reason and no note.
        let one = record(Some("Film"), &["Film"]);
        let main_is_title_1 = BTreeMap::from([(0, 1)]);
        let disc = java_disc(&[(1, "FPL_MainFeature_UR", &[ENG_AC3]), (2, "FPL_MainFeature_R", &[])]);
        assert_eq!(candidate_filter(&disc, &Settings::default()), vec![1]);
        let plan = by_database(&one, &main_is_title_1, &disc, &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(1, Some("Unrated Cut"), false, &[])]);
        assert_eq!((plan.confidence, plan.reviews, plan.notes), (0.98, vec![], vec![]));
    }

    #[test]
    fn the_title_rules_spell_themselves_as_the_spec_does() {
        assert_eq!(TitleRule::ALL.map(TitleRule::as_str), ["database", "labels", "structure"]);
        for rule in TitleRule::ALL {
            assert_eq!(serde_json::to_string(&rule).unwrap(), format!("\"{}\"", rule.as_str()));
            assert_eq!(rule.as_str().parse::<TitleRule>(), Ok(rule));
        }
        assert!("Database".parse::<TitleRule>().is_err());
    }

    #[test]
    fn the_title_confidences_and_bands_are_the_spec_constants() {
        assert_eq!(
            (DATABASE_TITLE_CONFIDENCE, LABELS_TITLE_CONFIDENCE, STRUCTURE_TITLE_CONFIDENCE, AMBIGUOUS_TITLE_CONFIDENCE),
            (0.98, 0.90, 0.85, 0.60)
        );
        assert_eq!((RIVAL_BAND, SEGMENT_DIFFERENCE_MAX, RUNTIME_SANITY_BAND), (0.10, 0.25, 0.10));
        assert_eq!(THEATRICAL_CUT, "Theatrical Cut");
    }

    /// A Blu-ray scan whose titles are (number, attributes, streams after the video); `java` adds the `MSG:1005` banner
    /// for 1.18.2 and an `MSG:3344` line.
    fn blu_ray(java: bool, titles: &[(u32, Vec<(u32, String)>, &[&[(u32, &str)]])]) -> Disc {
        let borrowed_attrs: Vec<Vec<(u32, &str)>> = titles
            .iter()
            .map(|(_, list, _)| list.iter().map(|(id, value)| (*id, value.as_str())).collect())
            .collect();
        let streams: Vec<Vec<(u32, &[(u32, &str)])>> = titles
            .iter()
            .map(|(_, _, others)| {
                let mut list: Vec<(u32, &[(u32, &str)])> = vec![(0, VIDEO)];
                for (index, stream) in others.iter().enumerate() {
                    list.push((index as u32 + 1, *stream));
                }
                list
            })
            .collect();
        let rows: Vec<(u32, &[(u32, &str)], &[(u32, &[(u32, &str)])])> = titles
            .iter()
            .enumerate()
            .map(|(index, (number, _, _))| (*number, borrowed_attrs[index].as_slice(), streams[index].as_slice()))
            .collect();
        if java {
            disc_of(&scan_text_with_messages("1.18.2", true, BLURAY, &rows))
        } else {
            disc_of(&scan_text(BLURAY, &rows))
        }
    }

    /// A Blu-ray scan with Java in use (`MSG:3344`) whose titles are (number, comment, audio streams), each 100 minutes
    /// long with 16 chapters, playlist `0000<number>.mpls` and segment map `<number>`.
    fn java_disc(titles: &[(u32, &str, &[&[(u32, &str)]])]) -> Disc {
        let titles: Vec<(u32, Vec<(u32, String)>, &[&[(u32, &str)]])> = titles
            .iter()
            .map(|(number, comment, audio)| {
                let attrs = vec![
                    (9, "1:40:00".to_string()),
                    (8, "16".to_string()),
                    (16, format!("{number:05}.mpls")),
                    (26, number.to_string()),
                    (49, comment.to_string()),
                ];
                (*number, attrs, *audio)
            })
            .collect();
        blu_ray(true, &titles)
    }

    #[test]
    fn expendables_labels_give_two_cuts_planned_as_editions() {
        assert_eq!(
            by_labels(&pinned(EXPENDABLES), &wanted_languages(None)),
            Some(MoviePlan {
                rule: TitleRule::Labels,
                features: vec![feature(41, Some("Unrated Cut"), false, &[]), feature(54, Some("Theatrical Cut"), true, &[])],
                feature_seconds: vec![Some(7891), Some(7587)],
                confidence: 0.90,
                reviews: vec![],
                notes: vec![],
                rivals: vec![],
                evidence: strings(&[
                    "label cut FPL_MainFeature_UR: titles [41], title 41 chosen",
                    "label cut FPL_MainFeature_R: titles [54], title 54 chosen",
                ]),
            })
        );
    }

    #[test]
    fn smile_label_gives_one_cut() {
        let plan = by_labels(&pinned(SMILE), &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(plan.features, vec![feature(14, Some("Theatrical Cut"), true, &[])]);
        assert_eq!((plan.confidence, plan.reviews), (LABELS_TITLE_CONFIDENCE, vec![]));
    }

    #[test]
    fn labels_are_trusted_only_when_java_is_in_use() {
        assert_eq!(by_labels(&pinned(JOHN_WICK_UHD), &wanted_languages(None)), None);
        assert_eq!(by_labels(&two_titles(false, "FPL_MainFeature_UR"), &wanted_languages(None)), None);
        assert!(by_labels(&two_titles(true, "FPL_MainFeature_UR"), &wanted_languages(None)).is_some());
    }

    #[test]
    fn a_java_disc_without_labels_gives_no_label_plan() {
        let no_label = java_disc(&[(1, "TMPL Main Menu", &[ENG_AC3]), (2, "SEG FPL_02", &[ENG_AC3])]);
        assert_eq!(by_labels(&no_label, &wanted_languages(None)), None);
        let narrative_only = java_disc(&[(1, "FPL_MainFeature_Narrative", &[ENG_AC3])]);
        assert_eq!(by_labels(&narrative_only, &wanted_languages(None)), None);
    }

    #[test]
    fn an_unnamed_cut_gives_0_60_and_unnamed_cut() {
        let disc = java_disc(&[(1, "FPL_MainFeature", &[ENG_AC3]), (2, "FPL_MainFeature_DC", &[ENG_AC3])]);
        let plan = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(plan.features, vec![feature(1, Some("Theatrical Cut"), true, &[]), feature(2, None, false, &[])]);
        assert_eq!((plan.confidence, plan.reviews), (AMBIGUOUS_TITLE_CONFIDENCE, vec![ReviewReason::UnnamedCut]));
    }

    #[test]
    fn a_lone_cut_with_an_other_suffix_is_unnamed_at_0_60() {
        // Spec 8.3 rule 2: a suffix the table does not name (`_IMAX`) is a cut with no edition name, even when it is the only cut.
        assert_eq!(
            by_labels(&java_disc(&[(1, "FPL_MainFeature_IMAX", &[ENG_AC3])]), &wanted_languages(None)),
            Some(MoviePlan {
                rule: TitleRule::Labels,
                features: vec![feature(1, None, false, &[])],
                feature_seconds: vec![Some(6000)],
                confidence: 0.60,
                reviews: vec![ReviewReason::UnnamedCut],
                notes: vec![],
                rivals: vec![],
                evidence: strings(&["label cut FPL_MainFeature_IMAX: titles [1], title 1 chosen"]),
            })
        );
    }

    #[test]
    fn within_a_cut_the_variant_covering_most_wanted_languages_is_used() {
        let disc = java_disc(&[
            (1, "FPL_MainFeature", &[ENG_AC3]),
            (2, "FPL_MainFeature_SPA", &[ENG_AC3, SPA_AC3]),
            (3, "FPL_MainFeature_JPN", &[JPN_AC3]),
        ]);
        // English only: titles 1 and 2 both cover English; the one without a language suffix wins.
        let english = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(english.features, vec![feature(1, Some("Theatrical Cut"), true, &[2, 3])]);
        assert_eq!(english.evidence, strings(&["label cut FPL_MainFeature: titles [1, 2, 3], title 1 chosen"]));
        // A Spanish film: title 2 covers English and Spanish.
        let spanish = by_labels(&disc, &wanted_languages(Some("es"))).expect("rule 2 applies");
        assert_eq!(spanish.features, vec![feature(2, Some("Theatrical Cut"), true, &[1, 3])]);
        assert_eq!(spanish.confidence, english.confidence);
    }

    #[test]
    fn a_dub_variant_of_a_named_cut_joins_that_cut() {
        // 8.3 rule 2 groups by suffix with the trailing language code removed: `_UR_JPN` is the `_UR` cut, not a cut of its own.
        let disc = java_disc(&[(1, "FPL_MainFeature_UR", &[ENG_AC3]), (2, "FPL_MainFeature_UR_JPN", &[JPN_AC3])]);
        let plan = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(plan.features, vec![feature(1, Some("Unrated Cut"), false, &[2])]);
        assert_eq!(plan.evidence, strings(&["label cut FPL_MainFeature_UR: titles [1, 2], title 1 chosen"]));
        assert_eq!(plan.confidence, 0.90);
    }

    #[test]
    fn within_a_cut_equal_variants_fall_to_the_lower_title_number() {
        let disc = java_disc(&[(3, "FPL_MainFeature_JPN", &[ENG_AC3]), (5, "FPL_MainFeature_SPA", &[ENG_AC3])]);
        let plan = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(plan.features, vec![feature(3, Some("Theatrical Cut"), true, &[5])]);
    }

    /// A two-letter English stream, as a disc's attribute 3 can carry it (`lang::is_english`).
    const EN_AC3: &[(u32, &str)] = &[(1, "Audio"), (3, "en"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];

    #[test]
    fn a_two_letter_english_code_counts_as_english_when_a_cut_chooses_its_variant() {
        // SR4: `coverage` reads English through `lang::is_english`, so "en" counts as "eng" does.
        let disc = java_disc(&[(1, "FPL_MainFeature", &[JPN_AC3]), (2, "FPL_MainFeature_SPA", &[EN_AC3])]);
        let plan = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(plan.features, vec![feature(2, Some("Theatrical Cut"), true, &[1])]);
    }

    #[test]
    fn narrative_variants_are_ignored() {
        let disc = java_disc(&[(1, "FPL_MainFeature", &[ENG_AC3]), (2, "FPL_MainFeature_Narrative", &[ENG_AC3])]);
        let plan = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(plan.features, vec![feature(1, Some("Theatrical Cut"), true, &[])]);
        assert_eq!(plan.confidence, 0.90);
    }

    #[test]
    fn cuts_are_ordered_by_their_lowest_title_number() {
        let disc = java_disc(&[
            (1, "FPL_MainFeature_JPN", &[JPN_AC3]),
            (2, "FPL_MainFeature_UR", &[ENG_AC3]),
            (5, "FPL_MainFeature", &[ENG_AC3]),
        ]);
        let plan = by_labels(&disc, &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(
            plan.features,
            vec![feature(5, Some("Theatrical Cut"), true, &[1]), feature(2, Some("Unrated Cut"), false, &[])]
        );
        assert_eq!(plan.feature_seconds, vec![Some(6000), Some(6000)]);
        assert_eq!((plan.confidence, plan.reviews), (0.90, vec![]));
    }

    #[test]
    fn two_cuts_with_the_same_edition_name_are_ambiguous() {
        // The table's `none` and `_R` rows are two cuts, both "Theatrical Cut": filed as editions they would collide.
        let disc = java_disc(&[(1, "FPL_MainFeature", &[ENG_AC3]), (2, "FPL_MainFeature_R", &[ENG_AC3])]);
        assert_eq!(
            by_labels(&disc, &wanted_languages(None)),
            Some(MoviePlan {
                rule: TitleRule::Labels,
                features: vec![feature(1, Some("Theatrical Cut"), true, &[]), feature(2, Some("Theatrical Cut"), true, &[])],
                feature_seconds: vec![Some(6000), Some(6000)],
                confidence: 0.60,
                reviews: vec![ReviewReason::TitleAmbiguous],
                notes: vec![],
                rivals: vec![],
                evidence: strings(&[
                    "label cut FPL_MainFeature: titles [1], title 1 chosen",
                    "label cut FPL_MainFeature_R: titles [2], title 2 chosen",
                    "two cuts are both named Theatrical Cut",
                ]),
            })
        );
    }

    /// The groups of the candidates under `settings`.
    fn groups_of(disc: &Disc, settings: &Settings) -> Vec<Group> {
        groups(disc, &candidate_filter(disc, settings))
    }

    /// Rule 3's plan for a disc that has no play-all title of episode-length parts (rule 3.1, ruling OR1); a play-all
    /// of shorter extras is an ordinary group here.
    fn structure(disc: &Disc, settings: &Settings) -> MoviePlan {
        match by_structure(disc, &groups_of(disc, settings), settings) {
            StructureOutcome::Plan(plan) => plan,
            other => panic!("expected a plan, got {other:?}"),
        }
    }

    /// A Blu-ray scan whose titles are (number, duration, chapters, segment map, streams after the video), each with
    /// playlist `0000<number>.mpls`.
    fn structure_disc(titles: &[(u32, &str, &str, &str, &[&[(u32, &str)]])]) -> Disc {
        let titles: Vec<(u32, Vec<(u32, String)>, &[&[(u32, &str)]])> = titles
            .iter()
            .map(|(number, duration, chapters, map, others)| {
                let mut list = vec![(9, duration.to_string()), (16, format!("{number:05}.mpls")), (26, map.to_string())];
                if !chapters.is_empty() {
                    list.push((8, chapters.to_string()));
                }
                (*number, list, *others)
            })
            .collect();
        blu_ray(false, &titles)
    }

    #[test]
    fn segment_difference_counts_segments_in_one_map_only_over_the_longer_length() {
        assert_eq!(segment_difference(&[], &[]), 0.0);
        assert_eq!(segment_difference(&[1, 2, 3], &[1, 2, 3]), 0.0);
        assert_eq!(segment_difference(&[1, 2, 3, 4], &[1, 2, 3, 5]), 0.5);
        assert_eq!(segment_difference(&[1, 2, 3, 4, 5, 6, 7, 8], &[1, 2, 3, 4, 5, 6, 7, 9]), 0.25);
        assert_eq!(segment_difference(&[1, 2], &[1, 2, 3, 4]), 0.5);
        assert_eq!(segment_difference(&[20], &[22]), 2.0);
        assert_eq!(segment_difference(&[], &[1]), 1.0);
    }

    #[test]
    fn john_wick_uhd_by_structure_takes_the_playlist_with_chapters() {
        assert_eq!(
            structure(&pinned(JOHN_WICK_UHD), &Settings::default()),
            MoviePlan {
                rule: TitleRule::Structure,
                features: vec![feature(0, None, true, &[8])],
                feature_seconds: vec![Some(6073)],
                confidence: 0.85,
                reviews: vec![],
                notes: vec![],
                rivals: strings(&["1"]),
                evidence: strings(&[
                    "groups at least 45 minutes long: 1 of 7",
                    "rival group 1: title 0 (the title with chapters)",
                ]),
            }
        );
    }

    #[test]
    fn knowing_by_structure_has_one_rival() {
        let plan = structure(&pinned(KNOWING), &Settings::default());
        assert_eq!(plan.features, vec![feature(0, None, true, &[])]);
        assert_eq!((plan.confidence, plan.reviews), (STRUCTURE_TITLE_CONFIDENCE, vec![]));
        assert_eq!(
            plan.evidence,
            strings(&["groups at least 45 minutes long: 1 of 3", "rival group 01:1-19: title 0 (its only title)"])
        );
    }

    #[test]
    fn a_disc_with_no_group_at_feature_length_keeps_the_longest() {
        let plan = structure(&pinned(FIREFLY_2), &Settings::default());
        assert_eq!(plan.features, vec![feature(0, None, true, &[])]);
        assert_eq!(plan.rivals, strings(&["01:1-5,6-9,10-14,15"]));
        assert_eq!((plan.confidence, plan.reviews), (0.85, vec![]));
        assert_eq!(
            plan.evidence,
            strings(&[
                "no group is at least 45 minutes long; the longest, 01:1-5,6-9,10-14,15, is kept",
                "rival group 01:1-5,6-9,10-14,15: title 0 (its only title)",
            ])
        );
    }

    #[test]
    fn a_longer_feature_min_minutes_keeps_only_the_longest_group() {
        let settings = Settings { feature_min_minutes: 150, ..Settings::default() };
        let plan = structure(&pinned(EXPENDABLES), &settings);
        assert_eq!(plan.features, vec![feature(58, None, true, &[])]);
        assert_eq!(plan.feature_seconds, vec![Some(7934)]);
        assert_eq!((plan.confidence, plan.reviews), (0.85, vec![]));
    }

    #[test]
    fn a_shorter_feature_min_minutes_makes_firefly_disc_2_four_rivals_that_cannot_be_told_apart() {
        let settings = Settings { feature_min_minutes: 40, ..Settings::default() };
        let plan = structure(&pinned(FIREFLY_2), &settings);
        assert_eq!(
            plan.features.iter().map(|feature| feature.title).collect::<Vec<_>>(),
            vec![0, 1, 2, 5]
        );
        assert_eq!((plan.confidence, plan.reviews), (AMBIGUOUS_TITLE_CONFIDENCE, vec![ReviewReason::TitleAmbiguous]));
        assert_eq!(
            plan.evidence.last().map(String::as_str),
            Some("4 rivals; the largest segment difference between two of them is 0.00")
        );
    }

    #[test]
    fn a_group_exactly_feature_min_minutes_long_is_kept() {
        let disc = structure_disc(&[(0, "0:45:00", "10", "1", &[ENG_AC3]), (1, "0:44:59", "10", "2", &[ENG_AC3])]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.rivals, strings(&["1"]));
        assert_eq!(plan.features, vec![feature(0, None, true, &[])]);
        assert_eq!(plan.evidence[0], "groups at least 45 minutes long: 1 of 2");
        assert_eq!(plan.confidence, 0.85);
    }

    #[test]
    fn rivals_are_within_10_percent_of_the_longest_kept_group_and_largely_different_maps_are_multiple_features() {
        let disc = structure_disc(&[
            (0, "1:40:00", "10", "1,2,3,4", &[ENG_AC3]),
            (1, "1:30:00", "10", "1,2,3,5", &[ENG_AC3]),
            (2, "1:29:59", "10", "9", &[ENG_AC3]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.rivals, strings(&["1,2,3,4", "1,2,3,5"]));
        assert_eq!(plan.features, vec![feature(0, None, true, &[]), feature(1, None, true, &[])]);
        assert_eq!((plan.confidence, plan.reviews), (0.60, vec![ReviewReason::MultipleFeatures]));
        assert_eq!(
            plan.evidence.last().map(String::as_str),
            Some("2 rivals; the largest segment difference between two of them is 0.50")
        );
    }

    #[test]
    fn rivals_differing_by_at_most_a_quarter_of_their_segments_are_ambiguous() {
        let disc = structure_disc(&[
            (0, "1:40:00", "10", "1,2,3,4,5,6,7,8", &[ENG_AC3]),
            (1, "1:40:00", "10", "1,2,3,4,5,6,7,9", &[ENG_AC3]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.features, vec![feature(0, None, true, &[]), feature(1, None, true, &[])]);
        assert_eq!((plan.confidence, plan.reviews), (0.60, vec![ReviewReason::TitleAmbiguous]));
    }

    #[test]
    fn expendables_by_structure_has_eight_rivals_that_are_largely_different() {
        let plan = structure(&pinned(EXPENDABLES), &Settings::default());
        assert_eq!(
            plan.features.iter().map(|feature| feature.title).collect::<Vec<_>>(),
            vec![38, 41, 54, 55, 56, 57, 58, 59]
        );
        assert_eq!(plan.rivals.len(), 8);
        assert_eq!((plan.confidence, plan.reviews.clone()), (0.60, vec![ReviewReason::MultipleFeatures]));
        assert_eq!(plan.evidence[0], "groups at least 45 minutes long: 9 of 29");
        assert_eq!(
            plan.evidence.last().map(String::as_str),
            Some("8 rivals; the largest segment difference between two of them is 1.00")
        );
    }

    #[test]
    fn within_a_rival_group_the_title_with_chapters_is_chosen() {
        let disc = structure_disc(&[
            (0, "1:40:00", "", "5", &[ENG_AC3, SPA_AC3, ENG_PGS]),
            (1, "1:40:00", "12", "5", &[ENG_AC3]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.features, vec![feature(1, None, true, &[0])]);
        assert_eq!(plan.confidence, 0.85);
        assert_eq!(plan.evidence[1], "rival group 5: title 1 (the title with chapters)");
    }

    #[test]
    fn within_a_rival_group_the_title_whose_streams_cover_the_others_is_chosen() {
        let disc = structure_disc(&[
            (0, "1:40:00", "12", "5", &[ENG_AC3, ENG_PGS]),
            (1, "1:40:00", "12", "5", &[ENG_AC3, SPA_AC3, ENG_PGS]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.features, vec![feature(1, None, true, &[0])]);
        assert_eq!((plan.confidence, plan.reviews), (0.85, vec![]));
        assert_eq!(plan.evidence[1], "rival group 5: title 1 (its streams cover every other title's)");
    }

    #[test]
    fn a_rival_group_where_no_title_covers_the_others_is_ambiguous() {
        let disc = structure_disc(&[
            (0, "1:40:00", "12", "5", &[ENG_AC3, FRA_AC3]),
            (1, "1:40:00", "12", "5", &[ENG_AC3, SPA_AC3]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.features, vec![feature(0, None, true, &[1])]);
        assert_eq!((plan.confidence, plan.reviews), (AMBIGUOUS_TITLE_CONFIDENCE, vec![ReviewReason::TitleAmbiguous]));
        assert_eq!(plan.evidence[1], "rival group 5: title 0 (no title's streams cover every other's)");
    }

    #[test]
    fn an_unchaptered_superset_does_not_win_over_chaptered_titles() {
        // Title 2's streams cover both others', but it has no chapters, so only titles 0 and 1 stay in the running (R2).
        let disc = structure_disc(&[
            (0, "1:40:00", "12", "5", &[ENG_AC3, FRA_AC3]),
            (1, "1:40:00", "12", "5", &[ENG_AC3, SPA_AC3]),
            (2, "1:40:00", "", "5", &[ENG_AC3, FRA_AC3, SPA_AC3]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.features, vec![feature(0, None, true, &[1, 2])]);
        assert_eq!((plan.confidence, plan.reviews), (AMBIGUOUS_TITLE_CONFIDENCE, vec![ReviewReason::TitleAmbiguous]));
        assert_eq!(plan.evidence[1], "rival group 5: title 0 (no title's streams cover every other's)");
    }

    #[test]
    fn a_title_with_one_more_stream_of_a_signature_covers_the_other() {
        // Rule 3.4 counts each signature: title 1 holds two English PGS streams (a regular and an SDH track, R5 and D20),
        // title 0 one, and title 1 comes second; then a second English Dolby Digital 5.1 track the same way.
        let disc = structure_disc(&[
            (0, "1:40:00", "12", "5", &[ENG_AC3, ENG_PGS]),
            (1, "1:40:00", "12", "5", &[ENG_AC3, ENG_PGS, ENG_PGS]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.features, vec![feature(1, None, true, &[0])]);
        assert_eq!((plan.confidence, plan.reviews), (0.85, vec![]));
        assert_eq!(plan.evidence[1], "rival group 5: title 1 (its streams cover every other title's)");
        let audio = structure_disc(&[(0, "1:40:00", "12", "5", &[ENG_AC3]), (1, "1:40:00", "12", "5", &[ENG_AC3, ENG_AC3])]);
        assert_eq!(structure(&audio, &Settings::default()).features, vec![feature(1, None, true, &[0])]);
    }

    #[test]
    fn titles_with_identical_streams_fall_to_the_first() {
        let disc = structure_disc(&[(0, "1:40:00", "12", "5", &[ENG_AC3]), (1, "1:40:00", "12", "5", &[ENG_AC3])]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!(plan.features, vec![feature(0, None, true, &[1])]);
        assert_eq!(plan.confidence, 0.85);
    }

    #[test]
    fn a_play_all_of_episode_length_parts_makes_the_structure_rule_answer_tv() {
        let disc = structure_disc(&[
            (0, "1:07:30", "15", "1,2,3", &[ENG_AC3]),
            (1, "0:22:00", "5", "1", &[ENG_AC3]),
            (2, "0:22:30", "5", "2", &[ENG_AC3]),
            (3, "0:23:00", "5", "3", &[ENG_AC3]),
        ]);
        let settings = Settings::default();
        assert_eq!(by_structure(&disc, &groups_of(&disc, &settings), &settings), StructureOutcome::PlayAll { group: 0 });
    }

    #[test]
    fn an_extras_play_all_is_not_rule_3_1s_play_all_title() {
        // OR1: the pinned logs' play-all titles (a deleted-scenes playlist, two extras playlists) have parts of a few minutes.
        for log in [CHERNOBYL_1, CHERNOBYL_2, SMILE] {
            let disc = pinned(log);
            let settings = Settings::default();
            assert!(matches!(by_structure(&disc, &groups_of(&disc, &settings), &settings), StructureOutcome::Plan(_)), "{log}");
        }
        let disc = structure_disc(&[
            (0, "1:40:00", "16", "1", &[ENG_AC3]),
            (1, "0:05:00", "", "2", &[ENG_AC3]),
            (2, "0:05:00", "", "3", &[ENG_AC3]),
            (3, "0:10:00", "", "2,3", &[ENG_AC3]),
        ]);
        let plan = structure(&disc, &Settings::default());
        assert_eq!((plan.features, plan.confidence), (vec![feature(0, None, true, &[])], 0.85));
    }

    #[test]
    fn chernobyl_2_without_thediscdb_is_held_because_its_two_episodes_are_rivals() {
        // Ruling OR1: without TheDiscDB, labels or a season marker, CHERNOBYL_2's extras play-all no longer makes it
        // TV. The disc is held in copy mode only because its two episodes (65 and 72 minutes) are within 10% of each
        // other, so rule 3 finds two rivals; episodes further apart beside an extras play-all would plan the longer one
        // alone at 0.85 (left to the owner, G1).
        let disc = pinned(CHERNOBYL_2);
        let settings = Settings::default();
        let found = groups_of(&disc, &settings);
        match movie_plan(&disc, None, &BTreeMap::new(), &found, &wanted_languages(None), &settings) {
            StructureOutcome::Plan(plan) => {
                assert_eq!((plan.rule, plan.confidence), (TitleRule::Structure, 0.60));
                assert_eq!(plan.reviews, vec![ReviewReason::MultipleFeatures]);
                assert_eq!(crate::plan::mode(plan.confidence, &settings), crate::plan::Mode::Copy);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_feature_whose_segments_are_listed_as_playlists_is_play_all_under_the_literal_reading() {
        // Review Focus 7: each of the feature's three segments is a playlist of its own lasting two minutes or more, so rule
        // 3.1 read literally calls the feature itself a play-all title and the movie would be planned as TV.
        let disc = structure_disc(&[
            (0, "1:40:00", "16", "1,2,3", &[ENG_AC3]),
            (1, "0:33:20", "4", "1", &[ENG_AC3]),
            (2, "0:33:20", "4", "2", &[ENG_AC3]),
            (3, "0:33:20", "4", "3", &[ENG_AC3]),
        ]);
        let settings = Settings::default();
        assert_eq!(by_structure(&disc, &groups_of(&disc, &settings), &settings), StructureOutcome::PlayAll { group: 0 });
    }

    #[test]
    fn a_disc_with_no_candidate_gives_an_ambiguous_empty_plan() {
        let disc = structure_disc(&[(0, "0:00:30", "", "1", &[ENG_AC3])]);
        let settings = Settings::default();
        assert_eq!(groups_of(&disc, &settings), vec![]);
        assert_eq!(
            by_structure(&disc, &[], &settings),
            StructureOutcome::Plan(MoviePlan {
                rule: TitleRule::Structure,
                features: vec![],
                feature_seconds: vec![],
                confidence: 0.60,
                reviews: vec![ReviewReason::TitleAmbiguous],
                notes: vec![],
                rivals: vec![],
                evidence: strings(&["no title passed the candidate filter"]),
            })
        );
    }

    /// A rule 3 plan with one feature per duration, at 0.85 and with no review.
    fn structure_plan(seconds: &[Option<u32>]) -> MoviePlan {
        MoviePlan {
            rule: TitleRule::Structure,
            features: (0..seconds.len()).map(|title| feature(title as u32, None, true, &[])).collect(),
            feature_seconds: seconds.to_vec(),
            confidence: STRUCTURE_TITLE_CONFIDENCE,
            reviews: vec![],
            notes: vec![],
            rivals: vec![],
            evidence: vec![],
        }
    }

    #[test]
    fn a_structure_feature_more_than_10_percent_from_the_runtime_drops_to_0_60() {
        let plan = structure(&pinned(JOHN_WICK_UHD), &Settings::default());
        assert_eq!(runtime_sanity(plan.clone(), Some(93)), plan);
        assert_eq!(runtime_sanity(plan.clone(), Some(112)), plan);
        for runtime in [92, 113] {
            let dropped = runtime_sanity(plan.clone(), Some(runtime));
            assert_eq!((dropped.confidence, dropped.reviews), (0.60, vec![ReviewReason::TitleAmbiguous]), "{runtime}");
            assert_eq!(
                dropped.evidence.last().cloned(),
                Some(format!("no feature is within 10% of the TMDb runtime of {runtime} minutes"))
            );
            assert_eq!(dropped.features, plan.features);
        }
    }

    #[test]
    fn runtime_sanity_holds_exactly_at_the_10_percent_edge() {
        // 100 minutes is 6000 seconds; 10% is 600.
        for seconds in [6600, 5400] {
            assert_eq!(runtime_sanity(structure_plan(&[Some(seconds)]), Some(100)), structure_plan(&[Some(seconds)]));
        }
        for seconds in [6601, 5399] {
            assert_eq!(runtime_sanity(structure_plan(&[Some(seconds)]), Some(100)).confidence, 0.60, "{seconds}");
        }
    }

    #[test]
    fn database_and_label_plans_and_unknown_runtimes_are_left_alone() {
        let database = database(EXPENDABLES, EXPENDABLES_API).expect("rule 1 applies");
        assert_eq!(runtime_sanity(database.clone(), Some(60)), database);
        let labels = by_labels(&pinned(EXPENDABLES), &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(runtime_sanity(labels.clone(), Some(60)), labels);
        let structural = structure_plan(&[Some(6000)]);
        assert_eq!(runtime_sanity(structural.clone(), None), structural);
        // TMDb's runtime 0 means it has none.
        assert_eq!(runtime_sanity(structural.clone(), Some(0)), structural);
        // A feature without a duration is never shown to be off.
        assert_eq!(runtime_sanity(structure_plan(&[None]), Some(100)), structure_plan(&[None]));
    }

    #[test]
    fn runtime_sanity_holds_while_any_feature_is_close() {
        let one_close = structure_plan(&[Some(6000), Some(9000)]);
        assert_eq!(runtime_sanity(one_close.clone(), Some(100)), one_close);
        let mut several = structure_plan(&[Some(9000), Some(3000)]);
        several.confidence = AMBIGUOUS_TITLE_CONFIDENCE;
        several.reviews = vec![ReviewReason::MultipleFeatures];
        let dropped = runtime_sanity(several, Some(100));
        assert_eq!(
            (dropped.confidence, dropped.reviews),
            (0.60, vec![ReviewReason::TitleAmbiguous, ReviewReason::MultipleFeatures])
        );
    }

    #[test]
    fn movie_plan_takes_the_first_applicable_rule() {
        let settings = Settings::default();
        let english = wanted_languages(None);
        let rule_of = |outcome: StructureOutcome| match outcome {
            StructureOutcome::Plan(plan) => Some(plan.rule),
            StructureOutcome::PlayAll { .. } => None,
        };
        let expendables = pinned(EXPENDABLES);
        let record = api_record(EXPENDABLES_API);
        let matches = match_titles(&record, &expendables);
        let found = groups_of(&expendables, &settings);
        assert_eq!(
            rule_of(movie_plan(&expendables, Some(&record), &matches, &found, &english, &settings)),
            Some(TitleRule::Database)
        );
        // Without TheDiscDB, or when its main movie does not reconcile, the labels decide.
        assert_eq!(
            rule_of(movie_plan(&expendables, None, &BTreeMap::new(), &found, &english, &settings)),
            Some(TitleRule::Labels)
        );
        assert_eq!(
            rule_of(movie_plan(&expendables, Some(&record), &BTreeMap::new(), &found, &english, &settings)),
            Some(TitleRule::Labels)
        );
        let john_wick = pinned(JOHN_WICK_UHD);
        let john_wick_groups = groups_of(&john_wick, &settings);
        assert_eq!(
            rule_of(movie_plan(&john_wick, None, &BTreeMap::new(), &john_wick_groups, &english, &settings)),
            Some(TitleRule::Structure)
        );
        let chernobyl = pinned(CHERNOBYL_1);
        let chernobyl_groups = groups_of(&chernobyl, &settings);
        assert_eq!(
            rule_of(movie_plan(&chernobyl, None, &BTreeMap::new(), &chernobyl_groups, &english, &settings)),
            Some(TitleRule::Structure)
        );
    }

    /// Library facts with earlier filed copies (cut key, edition name used).
    fn earlier(copies: &[(&str, &str)]) -> LibraryFacts {
        LibraryFacts {
            earlier_edition_names: copies.iter().map(|(key, name)| (key.to_string(), name.to_string())).collect(),
            ..LibraryFacts::default()
        }
    }

    /// Owned optional names.
    fn names(items: &[Option<&str>]) -> Vec<Option<String>> {
        items.iter().map(|item| item.map(str::to_string)).collect()
    }

    #[test]
    fn a_non_theatrical_cut_always_carries_its_edition_name_even_alone() {
        let alone = [feature(0, Some("Unrated Cut"), false, &[])];
        assert_eq!(edition_names(&alone, &LibraryFacts::default()), names(&[Some("Unrated Cut")]));
        assert_eq!(
            edition_names(&[feature(0, Some("Director's Cut"), false, &[])], &LibraryFacts::default()),
            names(&[Some("Director's Cut")])
        );
    }

    #[test]
    fn the_theatrical_cut_is_named_only_beside_another_cut_or_an_edition_named_file() {
        let alone = [feature(0, None, true, &[])];
        assert_eq!(edition_names(&alone, &LibraryFacts::default()), names(&[None]));
        let labelled_alone = [feature(0, Some("Theatrical Cut"), true, &[])];
        assert_eq!(edition_names(&labelled_alone, &LibraryFacts::default()), names(&[None]));
        let edition_named_folder = LibraryFacts { folder_has_edition_named_file: true, ..LibraryFacts::default() };
        assert_eq!(edition_names(&alone, &edition_named_folder), names(&[Some("Theatrical Cut")]));
        let two = [feature(0, None, true, &[]), feature(1, Some("Extended Cut"), false, &[])];
        assert_eq!(edition_names(&two, &LibraryFacts::default()), names(&[Some("Theatrical Cut"), Some("Extended Cut")]));
    }

    #[test]
    fn an_earlier_filed_copy_names_the_cut_the_same_way() {
        let alone = [feature(0, None, true, &[])];
        assert_eq!(edition_names(&alone, &earlier(&[("theatrical", "Theatrical Edition")])), names(&[Some("Theatrical Edition")]));
        let unrated = [feature(0, Some("Unrated Cut"), false, &[])];
        assert_eq!(edition_names(&unrated, &earlier(&[("unrated cut", "Unrated")])), names(&[Some("Unrated")]));
        // An earlier copy filed without an edition keeps this theatrical cut plain, even beside another cut.
        let two = [feature(0, None, true, &[]), feature(1, Some("Extended Cut"), false, &[])];
        assert_eq!(edition_names(&two, &earlier(&[("theatrical", "")])), names(&[None, Some("Extended Cut")]));
        // A copy of another cut changes nothing here.
        assert_eq!(edition_names(&alone, &earlier(&[("unrated cut", "Unrated")])), names(&[None]));
    }

    #[test]
    fn an_unnamed_cut_has_no_edition_name_until_review() {
        let two = [feature(0, None, true, &[]), feature(1, None, false, &[])];
        assert_eq!(edition_names(&two, &LibraryFacts::default()), names(&[Some("Theatrical Cut"), None]));
    }

    #[test]
    fn an_edition_naming_the_theatrical_cut_is_theatrical() {
        assert!(is_theatrical(None));
        assert!(is_theatrical(Some("Theatrical Cut")));
        assert!(is_theatrical(Some("theatrical edition")));
        assert!(!is_theatrical(Some("Unrated Cut")));
        assert!(!is_theatrical(Some("Theatrically Extended Cut")));
        // 8.4: TheDiscDB's "Theatrical Version" is the theatrical cut, so alone it is filed with no edition name.
        let one = record(Some("Film"), &["Film (Theatrical Version)"]);
        let plan = by_database(&one, &BTreeMap::from([(0, 0)]), &two_titles(false, "B1"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!(plan.features, vec![feature(0, Some("Theatrical Version"), true, &[])]);
        assert_eq!(edition_names(&plan.features, &LibraryFacts::default()), names(&[None]));
        // Beside a main movie with no edition it is a second theatrical cut: both would be filed "Theatrical Cut".
        let two = record(Some("Film"), &["Film", "Film (Theatrical)"]);
        let plan = by_database(&two, &BTreeMap::from([(0, 0), (1, 1)]), &two_titles(false, "B1"), &wanted_languages(None), &Settings::default()).expect("rule 1 applies");
        assert_eq!((plan.confidence, plan.reviews), (0.60, vec![ReviewReason::TitleAmbiguous]));
    }

    #[test]
    fn expendables_editions_are_named_from_its_plans() {
        let labels = by_labels(&pinned(EXPENDABLES), &wanted_languages(None)).expect("rule 2 applies");
        assert_eq!(
            edition_names(&labels.features, &LibraryFacts::default()),
            names(&[Some("Unrated Cut"), Some("Theatrical Cut")])
        );
        let database = database(EXPENDABLES, EXPENDABLES_API).expect("rule 1 applies");
        assert_eq!(edition_names(&database.features, &LibraryFacts::default()), names(&[Some("Unrated Cut"), None]));
    }
}
