//! Spec 8.1's lookups, `core::lookup`. Task 3 holds the answers: TheDiscDB's in `discdb` (spec 8.1, last bullet), with
//! the match of database titles to scan titles (spec 8.3, first paragraph), and TMDb's in `tmdb` (spec 8.1: the search,
//! the record with its external IDs, a season, the recording's slim `tmdb.json`). This file holds `Kind`, the disc kind
//! of spec 8.2, here because 8.1's identification and runtime checks are aware of it, and the two private helpers every
//! parser shares. Task 4 adds `name` (the disc's own name) and the identification of spec 8.1. No I/O: every parser
//! takes the answer's text and a name to show in its errors. Used by Tasks 4 to 7, 11, 13 and 15.

pub mod discdb;
pub mod tmdb;
/// The disc's own name: spec 8.1's parsing, cleaning, generic names and plausibility, and 8.2's season marker (Task 4).
pub mod name;

use std::str::FromStr;

/// The disc kind of spec 8.2 (TheDiscDB's "Movie" or "Series", or the structural decision); defined here because 8.1's
/// identification and runtime checks are kind-aware; `plan::kind` re-exports it. Spec 8.2. Def: T3. Use: T4 to T7,
/// T11, T13, T15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Movie,
    Tv,
}

impl Kind {
    /// Both kinds in spec 8.2's order: a movie (the default), then TV.
    pub const ALL: [Kind; 2] = [Kind::Movie, Kind::Tv];

    /// "movie" or "tv", the serde spelling. Spec 8.2.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Movie => "movie",
            Kind::Tv => "tv",
        }
    }
}

/// "movie" or "tv" exactly; any other text is refused with a sentence naming it. Spec 8.2.
impl FromStr for Kind {
    type Err = String;

    fn from_str(text: &str) -> Result<Kind, String> {
        Kind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| format!("{text:?} is not a disc kind (movie or tv)"))
    }
}

/// Movie to Movie, Tv to Tv (the two enums name the same two kinds; `TmdbKind` is TMDb's numbering space). Spec 8.1.
/// Def: T3. Use: T4, T7, T13.
impl From<tmdb::TmdbKind> for Kind {
    fn from(kind: tmdb::TmdbKind) -> Kind {
        match kind {
            tmdb::TmdbKind::Movie => Kind::Movie,
            tmdb::TmdbKind::Tv => Kind::Tv,
        }
    }
}

/// Movie to Movie, Tv to Tv: the TMDb kind a disc of this kind is searched and paired as. Spec 8.1. Def: T3. Use: T4,
/// T7, T13.
impl From<Kind> for tmdb::TmdbKind {
    fn from(kind: Kind) -> tmdb::TmdbKind {
        match kind {
            Kind::Movie => tmdb::TmdbKind::Movie,
            Kind::Tv => tmdb::TmdbKind::Tv,
        }
    }
}

/// An answer's text without the one byte-order mark a Windows recorder may write before it (the recorder's PowerShell
/// jobs write UTF-8 with one, as `recording::read_json` allows for).
fn without_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// An answer's JSON (after `without_bom`) as `T`; Err is serde_json's message after `shown` and ": ".
fn parse_json<T: serde::de::DeserializeOwned>(text: &str, shown: &str) -> Result<T, String> {
    serde_json::from_str(without_bom(text)).map_err(|error| format!("{shown}: {error}"))
}

// Spec 8.1's identification (Task 4). These items add no `use` line to this file: they name
// `crate::settings::Settings`, `crate::review::ReviewReason` and the child modules `discdb`, `tmdb` and `name` by
// path, so they never clash with the imports Task 3 wrote above.

/// Spec 8.1's table rows, after the owner's review: Reviewed is spec 11's Set identity (8.8 "A review decision sets the
/// confidences it covers to 1.00"). Def: T4. Use: T7, T13, T15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityStep {
    /// Spec 11's Set identity: the owner chose the identity (1.00, 8.8).
    Reviewed,
    /// Step 1: a remembered decision for this fingerprint (1.00).
    Remembered,
    /// Step 2: TheDiscDB match on both the disc ID and the fingerprint (0.99).
    DiscdbBoth,
    /// Step 3: TheDiscDB match on one of them (0.95).
    DiscdbOne,
    /// Step 4: TMDb search on the disc's own name, exactly one plausible result, and the runtime check passes (0.90,
    /// or 0.84 with identity_ambiguous when tmdb_only_autofile is off).
    TmdbOne,
    /// Step 5: several plausible results, or a failed runtime check (0.50, identity_ambiguous).
    TmdbSeveral,
    /// Step 6: nothing (0, identity_unknown).
    Nothing,
}

impl IdentityStep {
    /// The serde spelling (snake_case of the variant). Spec 8.1.
    pub fn as_str(self) -> &'static str {
        match self {
            IdentityStep::Reviewed => "reviewed",
            IdentityStep::Remembered => "remembered",
            IdentityStep::DiscdbBoth => "discdb_both",
            IdentityStep::DiscdbOne => "discdb_one",
            IdentityStep::TmdbOne => "tmdb_one",
            IdentityStep::TmdbSeveral => "tmdb_several",
            IdentityStep::Nothing => "nothing",
        }
    }

    /// Every step, in the order `identify` tries them. Spec 8.1.
    pub const ALL: [IdentityStep; 7] = [
        IdentityStep::Reviewed,
        IdentityStep::Remembered,
        IdentityStep::DiscdbBoth,
        IdentityStep::DiscdbOne,
        IdentityStep::TmdbOne,
        IdentityStep::TmdbSeveral,
        IdentityStep::Nothing,
    ];
}

impl std::str::FromStr for IdentityStep {
    type Err = String;

    /// Only the `as_str` spellings.
    fn from_str(text: &str) -> Result<IdentityStep, String> {
        IdentityStep::ALL
            .into_iter()
            .find(|step| step.as_str() == text)
            .ok_or_else(|| format!("'{text}' is not an identity step"))
    }
}

/// Step 1, and a review decision (8.8): 1.00. Def: T4. Use: T4, T7, T8, T10, T13.
pub const REMEMBERED_CONFIDENCE: f64 = 1.00;
/// Step 2. Def: T4. Use: T4.
pub const DISCDB_BOTH_CONFIDENCE: f64 = 0.99;
/// Step 3. Def: T4. Use: T4.
pub const DISCDB_ONE_CONFIDENCE: f64 = 0.95;
/// Step 4. Def: T4. Use: T4.
pub const TMDB_ONE_CONFIDENCE: f64 = 0.90;
/// Step 4 "When `tmdb_only_autofile` is off, step 4 is capped at 0.84 and adds `identity_ambiguous`". Def: T4. Use: T4.
pub const TMDB_ONE_CAPPED_CONFIDENCE: f64 = 0.84;
/// Step 5. Def: T4. Use: T4.
pub const TMDB_SEVERAL_CONFIDENCE: f64 = 0.50;
/// Step 6. Def: T4. Use: T4.
pub const NOTHING_CONFIDENCE: f64 = 0.0;

/// (confidence, review reason) of a step: Reviewed and Remembered 1.00; reads settings.tmdb_only_autofile for TmdbOne
/// (0.84 and IdentityAmbiguous when off); TmdbSeveral gives IdentityAmbiguous, Nothing gives IdentityUnknown. Spec 8.1
/// table, 8.8. Reads only `tmdb_only_autofile`. Def: T4. Use: T4, T7.
pub fn identity_confidence(
    step: IdentityStep,
    settings: &crate::settings::Settings,
) -> (f64, Option<crate::review::ReviewReason>) {
    use crate::review::ReviewReason;
    match step {
        IdentityStep::Reviewed | IdentityStep::Remembered => (REMEMBERED_CONFIDENCE, None),
        IdentityStep::DiscdbBoth => (DISCDB_BOTH_CONFIDENCE, None),
        IdentityStep::DiscdbOne => (DISCDB_ONE_CONFIDENCE, None),
        IdentityStep::TmdbOne if settings.tmdb_only_autofile => (TMDB_ONE_CONFIDENCE, None),
        IdentityStep::TmdbOne => (TMDB_ONE_CAPPED_CONFIDENCE, Some(ReviewReason::IdentityAmbiguous)),
        IdentityStep::TmdbSeveral => (TMDB_SEVERAL_CONFIDENCE, Some(ReviewReason::IdentityAmbiguous)),
        IdentityStep::Nothing => (NOTHING_CONFIDENCE, Some(ReviewReason::IdentityUnknown)),
    }
}

/// What the runtime checks compare against (8.1 "Runtime check"): the planned features' or cuts' durations (8.3, 8.4),
/// the longest rival when none is planned, and the durations of the disc's episode-length groups (8.2). The planner
/// fills it from the structural pass before the TMDb match (6.1 step 5's order). Def: T4. Use: T4, T7.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeFacts {
    /// The planned features or cuts, seconds, in plan order.
    pub feature_seconds: Vec<u32>,
    /// The longest rival, seconds, for a movie with no planned feature.
    pub longest_rival_seconds: Option<u32>,
    /// The episode-length groups (`plan::kind::episode_length_groups`), seconds, in group order.
    pub episode_group_seconds: Vec<u32>,
}

/// Spec 8.1 "within 3% of the TMDb runtime": a fixed value (spec 14), not a setting. Def: T4. Use: T4.
pub const MOVIE_RUNTIME_TOLERANCE: f64 = 0.03;
/// Spec 8.1 "within 15% of the median duration of the disc's episode-length groups": a fixed value. Def: T4. Use: T4.
pub const TV_RUNTIME_TOLERANCE: f64 = 0.15;

/// Movies: at least one of `feature_seconds` (else `longest_rival_seconds`) within 3% of `runtime_min * 60`; false
/// without a runtime or without any duration. Spec 8.1 "Runtime check for movies". Def: T4. Use: T4.
pub fn movie_runtime_ok(facts: &RuntimeFacts, runtime_min: Option<u32>) -> bool {
    let Some(minutes) = runtime_min else {
        return false;
    };
    let target = f64::from(minutes) * 60.0;
    let durations: Vec<u32> = if facts.feature_seconds.is_empty() {
        facts.longest_rival_seconds.into_iter().collect()
    } else {
        facts.feature_seconds.clone()
    };
    durations.iter().any(|&seconds| within(f64::from(seconds), target, MOVIE_RUNTIME_TOLERANCE))
}

/// TV: the median of `episode_runtimes_min` within 15% of the median of `episode_group_seconds / 60`; false when
/// either list is empty. The median of an even count is the mean of the two middle values. Spec 8.1 "For TV". Def: T4.
/// Use: T4.
pub fn tv_runtime_ok(facts: &RuntimeFacts, episode_runtimes_min: &[u32]) -> bool {
    let tmdb: Vec<f64> = episode_runtimes_min.iter().map(|&minutes| f64::from(minutes)).collect();
    let disc: Vec<f64> = facts.episode_group_seconds.iter().map(|&seconds| f64::from(seconds) / 60.0).collect();
    match (median_of(&tmdb), median_of(&disc)) {
        (Some(tmdb_median), Some(disc_median)) => within(tmdb_median, disc_median, TV_RUNTIME_TOLERANCE),
        _ => false,
    }
}

/// |a - b| <= tolerance * b. Spec 8.1's "within N% of" (b is the figure the percentage is of). Def: T4. Use: T4, T5
/// (rule 4's 10%), T6 (suggest_by_runtime's 15%).
pub fn within(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b
}

/// The median of `values`: the middle value of an odd count, the mean of the two middle values of an even count; None
/// for none. A separate copy on purpose: `audio::measures::py_median` is the Python-exact rule of the ported module
/// (RP6, RP20) and private to it, and `lazymkv-dev`'s `ocr_compare::median` lives in a crate core cannot call.
fn median_of(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let count = sorted.len();
    if count == 0 {
        None
    } else if count % 2 == 1 {
        Some(sorted[count / 2])
    } else {
        Some((sorted[count / 2 - 1] + sorted[count / 2]) / 2.0)
    }
}

/// A TheDiscDB description's meaning (8.1 last bullet, D14, 8.7 "the one TheDiscDB describes as subtitles or SDH"):
/// "comment" gives Commentary; "audio desc", "descriptive", "DVS", "visually impaired" give AudioDescription; "SDH",
/// "hearing" give Sdh; "isolated score", "score only" give IsolatedScore; "music and effects", "music & effects", "M&E"
/// give MusicAndEffects; "subtitle" (unless the text also names commentary: "Commentary subtitles" is Commentary only,
/// since a commentary description is not a plain one), or a description that after trimming and case-folding equals
/// the track's own `language` (TheDiscDB's language name: "English"; Knowing's "ENglish" included), gives Dialogue. All
/// case-insensitive, substring matches, in this order; a text can carry several. IsolatedScore, MusicAndEffects and
/// Dialogue are provisional: TheDiscDB's wording for them is not yet known (8.1's last bullet names no such word; every
/// subtitle description in the 18 answers and the pinned files is a bare language name). Def: T4. Use: T8, T9, T10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptionKind {
    Commentary,
    AudioDescription,
    Sdh,
    IsolatedScore,
    MusicAndEffects,
    Dialogue,
}

impl DescriptionKind {
    /// The serde spelling (snake_case of the variant). Spec 8.1.
    pub fn as_str(self) -> &'static str {
        match self {
            DescriptionKind::Commentary => "commentary",
            DescriptionKind::AudioDescription => "audio_description",
            DescriptionKind::Sdh => "sdh",
            DescriptionKind::IsolatedScore => "isolated_score",
            DescriptionKind::MusicAndEffects => "music_and_effects",
            DescriptionKind::Dialogue => "dialogue",
        }
    }

    /// Every kind, in the order `description_kinds` tests them. Spec 8.1.
    pub const ALL: [DescriptionKind; 6] = [
        DescriptionKind::Commentary,
        DescriptionKind::AudioDescription,
        DescriptionKind::Sdh,
        DescriptionKind::IsolatedScore,
        DescriptionKind::MusicAndEffects,
        DescriptionKind::Dialogue,
    ];
}

impl std::str::FromStr for DescriptionKind {
    type Err = String;

    /// Only the `as_str` spellings.
    fn from_str(text: &str) -> Result<DescriptionKind, String> {
        DescriptionKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| format!("'{text}' is not a description kind"))
    }
}

/// The phrases of each description kind, lower-case (spec 8.1 last bullet and D14; see DescriptionKind).
const COMMENTARY_PHRASES: [&str; 1] = ["comment"];
const AUDIO_DESCRIPTION_PHRASES: [&str; 4] = ["audio desc", "descriptive", "dvs", "visually impaired"];
const SDH_PHRASES: [&str; 2] = ["sdh", "hearing"];
const ISOLATED_SCORE_PHRASES: [&str; 2] = ["isolated score", "score only"];
const MUSIC_AND_EFFECTS_PHRASES: [&str; 3] = ["music and effects", "music & effects", "m&e"];
const DIALOGUE_PHRASES: [&str; 1] = ["subtitle"];

/// True when `text` contains one of `phrases`.
fn mentions(text: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| text.contains(*phrase))
}

/// The kinds `text` names, in DescriptionKind's order, each once; `language` is the track's own TheDiscDB `language`
/// ("English"), which a bare-language description equals. "subtitle" gives Dialogue unless the text also names
/// commentary (Open point 29: a plain subtitle description is Dialogue, and a commentary description is not plain;
/// 8.1's last bullet). Spec 8.1 last bullet, D14, 8.7. Def: T4. Use: T8, T9, T10.
pub fn description_kinds(text: &str, language: Option<&str>) -> Vec<DescriptionKind> {
    let folded = text.trim().to_lowercase();
    let own_language = language.map(|language| language.trim().to_lowercase()).filter(|language| !language.is_empty());
    let mut kinds = Vec::new();
    if mentions(&folded, &COMMENTARY_PHRASES) {
        kinds.push(DescriptionKind::Commentary);
    }
    if mentions(&folded, &AUDIO_DESCRIPTION_PHRASES) {
        kinds.push(DescriptionKind::AudioDescription);
    }
    if mentions(&folded, &SDH_PHRASES) {
        kinds.push(DescriptionKind::Sdh);
    }
    if mentions(&folded, &ISOLATED_SCORE_PHRASES) {
        kinds.push(DescriptionKind::IsolatedScore);
    }
    if mentions(&folded, &MUSIC_AND_EFFECTS_PHRASES) {
        kinds.push(DescriptionKind::MusicAndEffects);
    }
    let plain_subtitle = mentions(&folded, &DIALOGUE_PHRASES) && !kinds.contains(&DescriptionKind::Commentary);
    if plain_subtitle || own_language.as_deref() == Some(folded.as_str()) {
        kinds.push(DescriptionKind::Dialogue);
    }
    kinds
}

/// What the lookups gave (recorded, synthetic, or live in phase 2). `pending` flags say the answer is not in yet (8.1
/// "the lookup is recorded as pending"). Def: T4. Use: T7, T15.
#[derive(Clone, Debug, Default)]
pub struct LookupInputs<'a> {
    /// The disc's own name, built by the caller with `name::disc_name` (a generic name is signalled by
    /// `DiscName.generic`, not by None).
    pub name: Option<&'a name::DiscName>,
    /// `/api/discid/{id}`: the first record of the array that has titles, or None (a miss, or no disc ID: a DVD).
    pub discdb_by_disc_id: Option<&'a discdb::DiscDbRecord>,
    /// `/api/dischash/{hash}`.
    pub discdb_by_fingerprint: Option<&'a discdb::DiscDbRecord>,
    pub discdb_pending: bool,
    /// The search 8.1 runs on the cleaned name (or on TheDiscDB's title and year when the entry has no TMDb ID). When
    /// the disc name's `tagged` differs from its `cleaned` once both are folded (`name::fold_name`), the caller searches
    /// TMDb with both names and passes the two answer pages merged in order, the cleaned name's results first and then
    /// the tagged name's results not already among them, so the tagged match can be found (H1) and `PIRANHA_3D` keeps its
    /// fallback to Piranha. Spec 8.1, ruling J8.
    pub search: Option<&'a tmdb::TmdbSearch>,
    /// The record fetched for the identity in hand.
    pub record: Option<&'a tmdb::TmdbRecord>,
    pub tmdb_pending: bool,
    /// Step 1: the TMDb identity of a remembered decision for this fingerprint (`Remembered.tmdb`); plan::plan fills it
    /// from PlanInputs.remembered.
    pub remembered: Option<tmdb::TmdbIdentity>,
    /// The owner's Set identity (spec 11), before step 1; plan::plan fills it from PlanInputs.overrides.
    pub reviewed: Option<tmdb::TmdbIdentity>,
}

/// The identity 8.1 fixes at planning time. Def: T4. Use: T7, T11, T13, T15.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Identity {
    pub step: IdentityStep,
    pub confidence: f64,
    pub tmdb: Option<tmdb::TmdbIdentity>,
    /// From the record when fetched, else from TheDiscDB's media, else from the one plausible result.
    pub title: Option<String>,
    pub year: Option<i32>,
    pub original_language: Option<String>,
    pub review: Option<crate::review::ReviewReason>,
    /// The plausible results' identities (several in step 5; one in step 4).
    pub plausible: Vec<tmdb::TmdbIdentity>,
    /// Whether a runtime check ran and what it said (None: not applicable, no record, or no runtime).
    pub runtime_check: Option<bool>,
    /// Pending lookups (8.1: planning proceeds on structure alone; the file phase waits in AwaitingLookup).
    pub lookup_pending: bool,
    /// One sentence per piece of evidence, in order ("TheDiscDB knows the fingerprint", "name 'JOHN_WICK_UHD' cleaned to
    /// 'JOHN WICK'").
    pub evidence: Vec<String>,
}

/// Spec 8.1 steps 1 to 6 in order, after the owner's review: `inputs.reviewed` is Some (Reviewed, 1.00, that identity);
/// `inputs.remembered` is Some (Remembered, 1.00, that identity); TheDiscDB by both disc ID and fingerprint (0.99: both
/// records present and their TMDb IDs equal, or one of them lacking an ID) or by one (0.95); else the search on a
/// non-generic cleaned name (where a result matching the name with its region and format tags kept, `DiscName.tagged`,
/// is the only plausible one when there is such a result: spec 8.1, ruling H1): exactly one plausible result and the
/// runtime check passes (0.90, or 0.84 plus identity_ambiguous when tmdb_only_autofile is off); several plausible
/// results or a failed or impossible runtime check (0.50, identity_ambiguous); else nothing (0, identity_unknown). The
/// runtime check run is the one for the plausible result's own TmdbKind (movie_runtime_ok for Movie, tv_runtime_ok for
/// Tv), whatever `kind` is ("Movies and TV are searched together"); `kind` (the structural kind) is used only to prefer
/// results of that kind when several are plausible. A generic name is never searched. A pending lookup does not lower
/// the step reached by the answers in hand; it sets lookup_pending. A TheDiscDB entry without a TMDb ID is searched by
/// its title (`inputs.search` is that search): exactly one plausible result after preferring TheDiscDB's kind and then
/// its year keeps the TheDiscDB step with that result's identity, but one result whose kind or year contradicts
/// TheDiscDB's is step 5 with that identity; several give step 5, none step 6; when neither TheDiscDB record has a TMDb
/// ID the fingerprint's title is searched, unless only the disc ID record has one; an entry with neither a TMDb ID nor
/// a title falls through to the disc name's search. In the disc name's search, preferring the structural kind only
/// picks the result shown (`tmdb`, and the runtime check run on it): several plausible results stay step 5 (8.1 row 5),
/// and `plausible` lists them all. Reads only `settings.tmdb_only_autofile`. Def: T4. Use: T7.
pub fn identify(
    inputs: &LookupInputs<'_>,
    kind: Kind,
    runtime: &RuntimeFacts,
    settings: &crate::settings::Settings,
) -> Identity {
    let mut evidence = Vec::new();
    if inputs.discdb_pending {
        evidence.push("the TheDiscDB lookup is pending".to_string());
    }
    if inputs.tmdb_pending {
        evidence.push("the TMDb lookup is pending".to_string());
    }
    if let Some(disc_name) = inputs.name {
        evidence.push(format!("name '{}' cleaned to '{}'", disc_name.raw, disc_name.cleaned));
        if let Some(season) = disc_name.season {
            evidence.push(format!("season {season} parsed from the name"));
        }
        if let Some(disc) = disc_name.disc {
            evidence.push(format!("disc {disc} parsed from the name"));
        }
    }
    let found = find_identity(inputs, kind, runtime, &mut evidence);
    let (confidence, review) = identity_confidence(found.step, settings);
    let (title, year, original_language) = describe_identity(found.tmdb, inputs.record, found.media, found.result);
    Identity {
        step: found.step,
        confidence,
        tmdb: found.tmdb,
        title,
        year,
        original_language,
        review,
        plausible: found.plausible,
        runtime_check: found.runtime_check,
        lookup_pending: inputs.discdb_pending || inputs.tmdb_pending,
        evidence,
    }
}

/// The step `identify` reached and what it rests on: the TheDiscDB record whose media may name the work, and the one
/// plausible search result.
struct IdentityFound<'a> {
    step: IdentityStep,
    tmdb: Option<tmdb::TmdbIdentity>,
    media: Option<&'a discdb::DiscDbRecord>,
    result: Option<&'a tmdb::TmdbSearchResult>,
    plausible: Vec<tmdb::TmdbIdentity>,
    runtime_check: Option<bool>,
}

impl<'a> IdentityFound<'a> {
    /// `step` with nothing found yet.
    fn at(step: IdentityStep) -> IdentityFound<'a> {
        IdentityFound { step, tmdb: None, media: None, result: None, plausible: Vec::new(), runtime_check: None }
    }
}

/// "movie 245891", "tv 1437".
fn identity_text(identity: tmdb::TmdbIdentity) -> String {
    format!("{} {}", identity.kind.as_str(), identity.id)
}

/// The identities of `found`, joined by ", ".
fn identity_list(found: &[(tmdb::TmdbIdentity, &tmdb::TmdbSearchResult)]) -> String {
    found.iter().map(|(identity, _)| identity_text(*identity)).collect::<Vec<String>>().join(", ")
}

/// Steps 1 to 6 in order (see `identify`).
fn find_identity<'a>(
    inputs: &LookupInputs<'a>,
    kind: Kind,
    runtime: &RuntimeFacts,
    evidence: &mut Vec<String>,
) -> IdentityFound<'a> {
    if let Some(identity) = inputs.reviewed {
        evidence.push(format!("the owner set the identity to {}", identity_text(identity)));
        return IdentityFound {
            tmdb: Some(identity),
            media: discdb_media_for(inputs, identity),
            ..IdentityFound::at(IdentityStep::Reviewed)
        };
    }
    if let Some(identity) = inputs.remembered {
        evidence.push(format!("a remembered decision names {}", identity_text(identity)));
        return IdentityFound {
            tmdb: Some(identity),
            media: discdb_media_for(inputs, identity),
            ..IdentityFound::at(IdentityStep::Remembered)
        };
    }
    if let Some((step, record)) = discdb_match(inputs, evidence) {
        if let Some(identity) = record.tmdb_identity() {
            evidence.push(format!("TheDiscDB names {}", identity_text(identity)));
            return IdentityFound { tmdb: Some(identity), media: Some(record), ..IdentityFound::at(step) };
        }
        if let Some((title, year)) = record.title_and_year() {
            let with_year = year.map(|year| format!(" and year {year}")).unwrap_or_default();
            evidence.push(format!("TheDiscDB gives no TMDb ID: TMDb is searched with its title '{title}'{with_year}"));
            return search_by_entry(inputs, step, record, &title, year, evidence);
        }
        evidence.push("TheDiscDB gives neither a TMDb ID nor a title".to_string());
    }
    search_by_name(inputs, kind, runtime, evidence)
}

/// A TheDiscDB record in hand whose TMDb ID is `identity` (fingerprint first), for the title and year of a reviewed
/// or remembered identity.
fn discdb_media_for<'a>(inputs: &LookupInputs<'a>, identity: tmdb::TmdbIdentity) -> Option<&'a discdb::DiscDbRecord> {
    [inputs.discdb_by_fingerprint, inputs.discdb_by_disc_id]
        .into_iter()
        .flatten()
        .find(|record| record.tmdb_identity() == Some(identity))
}

/// Steps 2 and 3: the step and the record it rests on. Both records with different TMDb IDs give step 3 with the
/// fingerprint's record (the fingerprint is the primary key, spec 5 and 8.1), and the disagreement is written to the
/// evidence; both records with one or no ID give step 2 with the record that has the ID (the fingerprint's when
/// neither has one, unless only the disc ID record has a title).
fn discdb_match<'a>(
    inputs: &LookupInputs<'a>,
    evidence: &mut Vec<String>,
) -> Option<(IdentityStep, &'a discdb::DiscDbRecord)> {
    match (inputs.discdb_by_disc_id, inputs.discdb_by_fingerprint) {
        (Some(by_id), Some(by_fingerprint)) => {
            evidence.push("TheDiscDB knows the disc ID and the fingerprint".to_string());
            match (by_id.tmdb_identity(), by_fingerprint.tmdb_identity()) {
                (Some(of_id), Some(of_fingerprint)) if of_id != of_fingerprint => {
                    evidence.push(format!(
                        "TheDiscDB's disc ID record names {} and its fingerprint record {}: the fingerprint's is used",
                        identity_text(of_id),
                        identity_text(of_fingerprint)
                    ));
                    Some((IdentityStep::DiscdbOne, by_fingerprint))
                }
                (Some(_), None) => Some((IdentityStep::DiscdbBoth, by_id)),
                (None, None) if by_fingerprint.title_and_year().is_none() && by_id.title_and_year().is_some() => {
                    Some((IdentityStep::DiscdbBoth, by_id))
                }
                _ => Some((IdentityStep::DiscdbBoth, by_fingerprint)),
            }
        }
        (Some(by_id), None) => {
            evidence.push("TheDiscDB knows the disc ID".to_string());
            Some((IdentityStep::DiscdbOne, by_id))
        }
        (None, Some(by_fingerprint)) => {
            evidence.push("TheDiscDB knows the fingerprint".to_string());
            Some((IdentityStep::DiscdbOne, by_fingerprint))
        }
        (None, None) => None,
    }
}

/// The search results with an identity (a movie or a show) that are plausible for `text`, in the answer's order.
fn plausible_for<'a>(
    search: &'a tmdb::TmdbSearch,
    text: &str,
) -> Vec<(tmdb::TmdbIdentity, &'a tmdb::TmdbSearchResult)> {
    search
        .results
        .iter()
        .filter_map(|result| result.identity().map(|identity| (identity, result)))
        .filter(|(_, result)| name::plausible(result, text))
        .collect()
}

/// The evidence sentence naming the plausible results `found` of `search`; `tagged` is the name with its tags kept
/// when it decided which results are plausible (ruling H1).
fn plausible_sentence(
    found: &[(tmdb::TmdbIdentity, &tmdb::TmdbSearchResult)],
    search: &tmdb::TmdbSearch,
    tagged: Option<&str>,
) -> String {
    let mut sentence = format!("{} of {} search results are plausible", found.len(), search.results.len());
    if let Some(tagged) = tagged {
        sentence.push_str(&format!(" by the name with its tags kept, '{tagged}'"));
    }
    if !found.is_empty() {
        sentence.push_str(": ");
        sentence.push_str(&identity_list(found));
    }
    sentence
}

/// The search results with an identity (a movie or a show) that are plausible for `text`, in the answer's order, with
/// the evidence sentence naming them.
fn plausible_results<'a>(
    search: &'a tmdb::TmdbSearch,
    text: &str,
    evidence: &mut Vec<String>,
) -> Vec<(tmdb::TmdbIdentity, &'a tmdb::TmdbSearchResult)> {
    let found = plausible_for(search, text);
    evidence.push(plausible_sentence(&found, search, None));
    found
}

/// The results plausible for the disc's own name (spec 8.1, ruling H1): when the name with its tags kept differs from
/// the cleaned name once both are folded (`name::fold_name`, so `(US)` and `US` are one name) and some result matches
/// it, only those results (the more specific name wins: `PIRANHA_3D` is Piranha 3D, not Piranha); otherwise the results
/// matching the cleaned name. Writes one evidence sentence.
fn plausible_by_name<'a>(
    search: &'a tmdb::TmdbSearch,
    disc_name: &name::DiscName,
    evidence: &mut Vec<String>,
) -> Vec<(tmdb::TmdbIdentity, &'a tmdb::TmdbSearchResult)> {
    if name::fold_name(&disc_name.tagged) != name::fold_name(&disc_name.cleaned) {
        let found = plausible_for(search, &disc_name.tagged);
        if !found.is_empty() {
            evidence.push(plausible_sentence(&found, search, Some(&disc_name.tagged)));
            return found;
        }
    }
    plausible_results(search, &disc_name.cleaned, evidence)
}

/// Several plausible results narrowed to those of `kind`, then to those of `year`, each only when some but not all
/// match (see `identify`); the kind narrowing is skipped when `kind` is None (a TheDiscDB entry that states no kind,
/// whose search the structural kind never narrows). The disc name's search uses the result only to pick the identity
/// shown; the entry's search takes it as its answer.
fn prefer_results<'a>(
    found: Vec<(tmdb::TmdbIdentity, &'a tmdb::TmdbSearchResult)>,
    kind: Option<Kind>,
    year: Option<i32>,
    evidence: &mut Vec<String>,
) -> Vec<(tmdb::TmdbIdentity, &'a tmdb::TmdbSearchResult)> {
    let mut picked = found;
    if let (true, Some(kind)) = (picked.len() > 1, kind) {
        let wanted = tmdb::TmdbKind::from(kind);
        let of_kind: Vec<(tmdb::TmdbIdentity, &tmdb::TmdbSearchResult)> =
            picked.iter().copied().filter(|(identity, _)| identity.kind == wanted).collect();
        if !of_kind.is_empty() && of_kind.len() < picked.len() {
            evidence.push(format!("preferring {} results leaves {}", wanted.as_str(), identity_list(&of_kind)));
            picked = of_kind;
        }
    }
    if let (true, Some(year)) = (picked.len() > 1, year) {
        let of_year: Vec<(tmdb::TmdbIdentity, &tmdb::TmdbSearchResult)> =
            picked.iter().copied().filter(|(_, result)| result.year() == Some(year)).collect();
        if !of_year.is_empty() && of_year.len() < picked.len() {
            evidence.push(format!("preferring results from {year} leaves {}", identity_list(&of_year)));
            picked = of_year;
        }
    }
    picked
}

/// A TheDiscDB entry without a TMDb ID, searched by its title and year (8.1 "If the entry has no TMDb ID, TMDb is
/// searched with the entry's title and year"). TMDb's `/search/multi` takes no year, so the year is checked here: one
/// plausible result whose kind or year contradicts what TheDiscDB states (8.2 "TheDiscDB states the kind") is step 5
/// with that result's identity, never the TheDiscDB step.
fn search_by_entry<'a>(
    inputs: &LookupInputs<'a>,
    step: IdentityStep,
    record: &'a discdb::DiscDbRecord,
    title: &str,
    year: Option<i32>,
    evidence: &mut Vec<String>,
) -> IdentityFound<'a> {
    let Some(search) = inputs.search else {
        evidence.push("no TMDb search answer is in hand".to_string());
        return IdentityFound::at(IdentityStep::Nothing);
    };
    let found = plausible_results(search, title, evidence);
    let picked = prefer_results(found, record.kind(), year, evidence);
    match picked.as_slice() {
        [] => IdentityFound::at(IdentityStep::Nothing),
        [(identity, result)] => {
            let wrong_kind = record.kind().is_some_and(|stated| tmdb::TmdbKind::from(stated) != identity.kind);
            let wrong_year = matches!((year, result.year()), (Some(stated), Some(found)) if stated != found);
            if wrong_kind || wrong_year {
                evidence.push(format!("{} is not TheDiscDB's kind and year", identity_text(*identity)));
                IdentityFound {
                    tmdb: Some(*identity),
                    result: Some(*result),
                    plausible: vec![*identity],
                    ..IdentityFound::at(IdentityStep::TmdbSeveral)
                }
            } else {
                IdentityFound {
                    tmdb: Some(*identity),
                    media: Some(record),
                    result: Some(*result),
                    plausible: vec![*identity],
                    ..IdentityFound::at(step)
                }
            }
        }
        several => IdentityFound {
            plausible: several.iter().map(|(identity, _)| *identity).collect(),
            ..IdentityFound::at(IdentityStep::TmdbSeveral)
        },
    }
}

/// Steps 4 to 6: the search on the disc's own cleaned name. Step 4 needs exactly one plausible result (8.1 row 4):
/// when several are plausible, preferring the structural kind may pick the one shown, but the step stays 5 (row 5).
/// A result matching the name with its tags kept is the only plausible one (`plausible_by_name`, ruling H1).
fn search_by_name<'a>(
    inputs: &LookupInputs<'a>,
    kind: Kind,
    runtime: &RuntimeFacts,
    evidence: &mut Vec<String>,
) -> IdentityFound<'a> {
    let Some(disc_name) = inputs.name else {
        evidence.push("the disc has no name: TMDb is not searched".to_string());
        return IdentityFound::at(IdentityStep::Nothing);
    };
    if disc_name.generic {
        evidence.push(format!("name '{}' is generic: TMDb is not searched", disc_name.raw));
        return IdentityFound::at(IdentityStep::Nothing);
    }
    let Some(search) = inputs.search else {
        evidence.push("no TMDb search answer is in hand".to_string());
        return IdentityFound::at(IdentityStep::Nothing);
    };
    let found = plausible_by_name(search, disc_name, evidence);
    let several = found.len() > 1;
    let every_plausible: Vec<tmdb::TmdbIdentity> = found.iter().map(|(identity, _)| *identity).collect();
    let picked = prefer_results(found, Some(kind), None, evidence);
    match picked.as_slice() {
        [] => IdentityFound::at(IdentityStep::Nothing),
        [(identity, result)] => {
            let runtime_check = check_runtime(*identity, inputs.record, runtime, disc_name.season, evidence);
            let step = if runtime_check == Some(true) && !several {
                IdentityStep::TmdbOne
            } else {
                IdentityStep::TmdbSeveral
            };
            IdentityFound {
                tmdb: Some(*identity),
                result: Some(*result),
                plausible: every_plausible,
                runtime_check,
                ..IdentityFound::at(step)
            }
        }
        _ => IdentityFound { plausible: every_plausible, ..IdentityFound::at(IdentityStep::TmdbSeveral) },
    }
}

/// The runtime check of `identity`'s own kind over its fetched record: None when no record of that identity is in
/// hand, or when the record has no runtime (a movie) or no episode runtime for the parsed season, or for the whole
/// series when none was parsed (a show). The evidence sentence of the last case names the season it looked in.
fn check_runtime(
    identity: tmdb::TmdbIdentity,
    record: Option<&tmdb::TmdbRecord>,
    runtime: &RuntimeFacts,
    season: Option<u32>,
    evidence: &mut Vec<String>,
) -> Option<bool> {
    let Some(record) = record.filter(|record| record.identity == identity) else {
        evidence.push(format!("no runtime check: no TMDb record for {}", identity_text(identity)));
        return None;
    };
    let outcome = match identity.kind {
        tmdb::TmdbKind::Movie => record.runtime.map(|minutes| movie_runtime_ok(runtime, Some(minutes))),
        tmdb::TmdbKind::Tv => {
            let minutes = record.episode_runtimes(season);
            if minutes.is_empty() {
                None
            } else {
                Some(tv_runtime_ok(runtime, &minutes))
            }
        }
    };
    evidence.push(match outcome {
        Some(true) => "the runtime check passes".to_string(),
        Some(false) => "the runtime check fails".to_string(),
        None => match (identity.kind, season) {
            (tmdb::TmdbKind::Movie, _) => {
                format!("no runtime check: the TMDb record for {} has no runtime", identity_text(identity))
            }
            (tmdb::TmdbKind::Tv, Some(n)) => format!(
                "no runtime check: the TMDb record for {} has no episode runtime for season {n}",
                identity_text(identity)
            ),
            (tmdb::TmdbKind::Tv, None) => {
                format!("no runtime check: the TMDb record for {} has no episode runtime", identity_text(identity))
            }
        },
    });
    outcome
}

/// Title, year and original language of the identity reached: from the fetched record when its identity is that one,
/// else from TheDiscDB's media, else from the one plausible result; the original language only from the record.
fn describe_identity(
    identity: Option<tmdb::TmdbIdentity>,
    record: Option<&tmdb::TmdbRecord>,
    media: Option<&discdb::DiscDbRecord>,
    result: Option<&tmdb::TmdbSearchResult>,
) -> (Option<String>, Option<i32>, Option<String>) {
    let Some(identity) = identity else {
        return (None, None, None);
    };
    if let Some(record) = record.filter(|record| record.identity == identity) {
        return (Some(record.title.clone()), record.year, record.original_language.clone());
    }
    if let Some((title, year)) = media.and_then(|media| media.title_and_year()) {
        return (Some(title), year, None);
    }
    if let Some(result) = result {
        let title = result.display_title();
        return ((!title.is_empty()).then(|| title.to_string()), result.year(), None);
    }
    (None, None, None)
}

#[cfg(test)]
mod tests;
