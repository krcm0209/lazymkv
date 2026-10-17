//! The disc phase's planning (spec 6.1 step 5): the disc kind and its groups (8.2, `kind`), the movie titles and
//! their editions (8.3 rules 1 to 4 and 8.4, `movie`), the TV titles and season sessions (8.5, `tv`), the mode the
//! title confidence sets (6.2, 6.3, 8.8, `Mode`), and the planner itself, `plan`: it runs those rules and the
//! identification (8.1) in 6.1 step 5's order, applies remembered decisions and the owner's review decisions (spec 5,
//! 11; each sets the confidence it covers to 1.00, 8.8), orders and keys the titles (spec 5's title key), fixes the
//! identity and title confidences and gathers the review reasons and notes. `staging_bytes_needed` and
//! `write_bytes_needed` are section 7's space checks. Used by the decision (Task 13) and by `lazymkv-dev plan` and
//! `decide` (Tasks 15 and 16).

pub mod kind;
pub mod movie;
pub mod tv;

pub use crate::lookup::Kind;
pub use kind::{Group, KindSource};

use std::collections::BTreeMap;

use crate::disc::{Disc, Title};
use crate::lang::{wanted_languages, Wanted};
use crate::lookup::discdb::{match_titles, DiscDbRecord};
use crate::lookup::name::DiscName;
use crate::lookup::tmdb::{TmdbIdentity, TmdbKind, TmdbRecord};
use crate::lookup::{identify, Identity, IdentityStep, LookupInputs, RuntimeFacts, REMEMBERED_CONFIDENCE};
use crate::plan::movie::{is_theatrical, Feature, LibraryFacts, MoviePlan, StructureOutcome, TitleRule};
use crate::plan::tv::{Episode, EpisodeTitle, Found, SeasonSession, TvPlan};
use crate::remembered::{Overrides, Remembered, TitleRole};
use crate::review::{sorted_unique, Note, Noted, ReviewReason};
use crate::settings::Settings;

/// 6.2 or 6.3: Direct rips the planned titles; Copy backs up the whole disc and decides after review. Spec 6.2, 6.3, 8.8.
/// Def: T5. Use: T7, T14, T15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Direct,
    Copy,
}

impl Mode {
    /// Both modes, Direct first.
    pub const ALL: [Mode; 2] = [Mode::Direct, Mode::Copy];

    /// The serde spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Direct => "direct",
            Mode::Copy => "copy",
        }
    }
}

impl std::str::FromStr for Mode {
    type Err = String;

    /// Only the `as_str` spellings.
    fn from_str(text: &str) -> Result<Mode, String> {
        Mode::ALL.into_iter().find(|mode| mode.as_str() == text).ok_or_else(|| format!("'{text}' is not a mode"))
    }
}

/// Direct when `title_confidence >= settings.auto_threshold`, else Copy (6.2, 6.3, 8.8). Reads `settings.auto_threshold`
/// only. Def: T5. Use: T7.
pub fn mode(title_confidence: f64, settings: &Settings) -> Mode {
    if title_confidence >= settings.auto_threshold {
        Mode::Direct
    } else {
        Mode::Copy
    }
}

/// Spec 5: `t<NN>-<source file stem>` ("t00-00800") or `t<NN>-title<attribute 24>` on a DVD ("t00-title01"); `order` is the
/// title's position in `Plan.titles`. `NN` is `order` written with at least two digits; attribute 24 keeps MakeMKV's
/// spelling ("01", Open point 18), so keys stay stable across scans of the same disc. A Blu-ray title without attribute 16,
/// or a DVD title without attribute 24, is keyed `title<scan title number, two digits>` ("t02-title07"). Def: T7.
/// Use: T7, T13, T16.
pub fn title_key(order: usize, title: &Title, dvd: bool) -> String {
    let source = if dvd {
        title.dvd_title.as_deref().map(str::trim).filter(|number| !number.is_empty()).map(|number| format!("title{number}"))
    } else {
        title.source_file.as_deref().map(file_stem).filter(|stem| !stem.is_empty()).map(str::to_string)
    };
    let source = source.unwrap_or_else(|| format!("title{:02}", title.number));
    format!("t{order:02}-{source}")
}

/// A source file's name without its folder and without its last extension ("00800.mpls" gives "00800", "00001.m2ts"
/// gives "00001"); a name that starts with its only dot is kept whole.
fn file_stem(source_file: &str) -> &str {
    let name = source_file.rsplit(|c: char| c == '/' || c == '\\').next().unwrap_or(source_file);
    match name.rfind('.') {
        Some(at) if at > 0 => &name[..at],
        _ => name,
    }
}

/// A planned title's role (the planner's view; remembered::TitleRole is the stored one): a feature or cut with the edition
/// name 8.4 files it under (None for a lone theatrical cut), or an episode assignment (8.5). Spec 6.1 step 5. Def: T7.
/// Use: T11, T13, T15.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "role")]
pub enum PlannedRole {
    /// A feature or cut (8.3, 8.4); `edition` is the name `movie::edition_names` gives it.
    Feature { edition: Option<String> },
    /// An episode title (8.5).
    Episode(Episode),
}

/// One title of the plan, in plan order (spec 5's title key carries the position). Spec 6.1 step 5. Def: T7.
/// Use: T13, T15, T16.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlannedTitle {
    /// `title_key(position in Plan.titles, ..)`.
    pub key: String,
    /// The scan's title number.
    pub title: u32,
    pub role: PlannedRole,
    /// Attribute 9 in seconds.
    pub seconds: Option<u32>,
    /// Attribute 11 in bytes (section 7's Direct-mode space check adds them).
    pub size: Option<u64>,
    /// Attribute 16; None on a DVD.
    pub source_file: Option<String>,
    /// From 8.5's unmapped suggestions (never true for a remembered or reviewed title).
    pub suggested: bool,
    /// From a remembered decision or the owner's Assign titles (1.00).
    pub remembered: bool,
}

/// One pair of `lookup::discdb::match_titles`: a TheDiscDB title (its database index, the key) and the scan title it
/// matched. Spec 8.3 "Database titles are matched to scan titles". Def: T7. Use: T15, T16.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbMatch {
    /// The TheDiscDB title's index in the record (`DiscDbTitle.index`).
    pub discdb_index: u32,
    /// The scan title number it matched.
    pub title: u32,
}

/// What the planner saw, for the document and review (6.1 step 5 "Persist the plan with its evidence"). Def: T7. Use: T15.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanEvidence {
    /// The disc's own name as the caller built it (`LookupInputs.name`).
    pub name: Option<DiscName>,
    /// 8.2's candidate filter, in title order.
    pub candidates: Vec<u32>,
    /// 8.2's groups of the candidates, a play-all group included.
    pub groups: Vec<Group>,
    /// 8.3 rule 3's rival groups (keys); empty for rules 1 and 2 and for TV.
    pub rivals: Vec<String>,
    /// The 8.3 rule that chose the movie titles; None for TV.
    pub title_rule: Option<TitleRule>,
    /// Every title comment (attribute 49) that is an 8.3 rule 2 label, whether or not Java was in use: (title, comment).
    pub labels: Vec<(u32, String)>,
    /// `lookup::discdb::match_titles` of the TheDiscDB record the plan used: one entry per matched database title, sorted
    /// by database index. A list of entries, not a map keyed by the index: the plan is read back inside `decide`'s
    /// untagged sections, whose buffered content cannot give an integer map key (the interfaces' "Keyed collections").
    pub discdb_matches: Vec<DiscDbMatch>,
    /// One plain sentence per step, in the order the planner took them.
    pub sentences: Vec<String>,
}

/// The plan document of 6.1 step 5. Def: T7. Use: T13, T15, T16.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    pub identity: Identity,
    pub kind: Kind,
    pub kind_source: KindSource,
    pub titles: Vec<PlannedTitle>,
    pub mode: Mode,
    /// `identity.confidence`: fixed here (8.8).
    pub identity_confidence: f64,
    /// The title rule's confidence after rule 4, or 1.00 for remembered or assigned titles (8.8); held at 0.60 when a
    /// search identity's kind contradicts the planned kind (`plan`'s provisional rule).
    pub title_confidence: f64,
    /// Sorted and unique: the identity's review reason, the title rule's, and the kind disagreement's
    /// (identity_ambiguous, with title_ambiguous unless assigned titles cover the titles) (8.8).
    pub reviews: Vec<ReviewReason>,
    pub notes: Vec<Noted>,
    /// `wanted_languages` of the identity's original language (R4, 8.6 "Languages").
    pub wanted: Wanted,
    pub session: Option<SeasonSession>,
    pub evidence: PlanEvidence,
}

/// Everything the disc phase has at step 5. The disc name is `lookups.name` (the caller builds it once with
/// `lookup::name::disc_name` and searched with it). Def: T7. Use: T15.
pub struct PlanInputs<'a> {
    pub disc: &'a Disc,
    pub fingerprint: Option<&'a str>,
    pub disc_id: Option<&'a str>,
    pub lookups: LookupInputs<'a>,
    /// A remembered decision for this fingerprint (spec 5, 6.1 step 2 "A new job applies any remembered decisions").
    pub remembered: Option<&'a Remembered>,
    /// The owner's review decisions, when the plan is made again after review (phase 2 re-plans a copy job on approval).
    pub overrides: Option<&'a Overrides>,
    pub sessions: &'a [SeasonSession],
    pub library: &'a LibraryFacts,
    pub settings: &'a Settings,
}

/// 6.1 step 5 in order: candidates and groups (8.2); the kind; the structural pass (8.3 rules 1 to 3, or 8.5) to fill
/// RuntimeFacts, running by_labels with `wanted_languages(None)` (8.3 rule 3.1's play-all makes the disc TV unless
/// TheDiscDB states it is a movie, 8.2, when rule 3 runs without 3.1 over every group, `movie::by_structure_as_movie`,
/// so a play-all-shaped group is an ordinary rival; every play-all group is left out of the episode-length groups, 8.5
/// "play all titles are dropped"); identify (8.1) over `inputs.lookups` with
/// `remembered` set to `inputs.remembered.map(|r| r.tmdb)` and `reviewed` to `inputs.overrides.and_then(|o|
/// o.identity)`; when `identity.original_language` is Some and not English, by_labels runs again with
/// `wanted_languages(identity.original_language)` and its features replace the first pass's (cuts, editions and
/// confidence never change between the passes, only the variant chosen within a cut); `Plan.wanted` is
/// `wanted_languages(identity.original_language)` for every title rule (English only when the original language is
/// unknown or English); rule 4; a remembered decision (`remembered::apply_titles`) or the owner's Assign titles
/// (`overrides.titles`, which win) replace the planned titles and set title confidence to 1.00, and Set identity or a
/// remembered identity sets identity confidence to 1.00; `overrides.season` fills a planned episode whose season is
/// None (else the disc name's season does); the mode; reviews and notes gathered (8.8: identity and title confidence
/// are fixed here). `Plan.titles` order (which fixes the keys' NN through `title_key(position, ..)`): movies in feature
/// order (database index order for rule 1, each cut's lowest title number for rule 2, group order for rule 3), TV by
/// (season, first episode) with season None last in title order. The identity is confirmed when its step is Reviewed,
/// Remembered, DiscdbBoth or DiscdbOne (8.5 "when its identity is confirmed (by TheDiscDB or in review)"). With a mapped
/// TV plan and the identity confirmed (or assigned episodes), a TMDb show identity and a season, `Plan.session` is
/// `tv::find(..)`'s session or `SeasonSession::new(show_id, season, the record's episode_count for that season)`, with
/// the plan's episodes assigned (`assign` with the fingerprint, when given); with an unmapped plan the found session
/// gives `start` (and the season when none was parsed), and when the identity is confirmed and the season is known,
/// `Plan.session` is that session or a new one with the disc recorded and no episode assigned (spec 11 Set identity
/// "attaches the disc to a season session"); None otherwise. A search identity (TmdbOne, TmdbSeveral) whose kind is not
/// the planned kind adds identity_ambiguous and, unless assigned titles cover the titles (8.8: they stay at 1.00), holds
/// the title confidence at 0.60 with title_ambiguous, so whole-disc copy mode applies and review never needs the disc
/// again (6.3, 8.8 "Limits of D1"; a provisional rule: the spec leaves this case to the owner, and the final review's
/// ruling amends the brief's settled point 12). Err only when
/// the disc has no title at all. Assigned titles replace the planned ones whole (the owner's Assign titles when it names
/// any title, else the remembered titles when the scan matches every one of them that is not Ignored): their review
/// reasons go with them, a title they name that the scan lacks is left out, and they make the disc TV when they assign
/// only episodes and a movie when they assign only features (a sentence says so when the kind changes); assigned
/// episodes with a TMDb show identity join the season session as a mapped plan's do (8.5 "or in review"). Def: T7.
/// Use: T15.
pub fn plan(inputs: &PlanInputs<'_>) -> Result<Plan, String> {
    let disc = inputs.disc;
    let settings = inputs.settings;
    if disc.titles.is_empty() {
        return Err(NO_TITLE.to_string());
    }
    let name = inputs.lookups.name;
    let mut sentences = input_sentences(inputs);

    // 8.3 first paragraph: the TheDiscDB record the scan's titles are matched against, the fingerprint's first (8.1).
    let discdb: Option<&DiscDbRecord> = inputs.lookups.discdb_by_fingerprint.or(inputs.lookups.discdb_by_disc_id);
    let matches = discdb.map(|record| match_titles(record, disc)).unwrap_or_default();
    sentences.push(discdb_sentence(&inputs.lookups, matches.len()));

    // 8.2: the candidates, their groups and the kind.
    let candidates = kind::candidate_filter(disc, settings);
    let groups = kind::groups(disc, &candidates);
    sentences.push(format!(
        "{} of {} titles pass the candidate filter, in {} groups",
        candidates.len(),
        disc.titles.len(),
        groups.len()
    ));
    let (mut planned_kind, mut kind_source) = kind::disc_kind(discdb, &groups, name);
    sentences.push(format!("the disc is {}: {}", kind_words(planned_kind), kind_source_words(kind_source)));
    // 8.5 "play all titles are dropped": no play-all group (8.3 rule 3.1) is an episode, so each is left out until none is.
    let (episode_groups, _) = tv::without_play_all(&groups, disc.is_dvd);

    // 8.3 rules 1 to 3 with English only (the original language comes with the identity), or 8.5's map.
    let english = wanted_languages(None);
    let mut found_movie: Option<MoviePlan> = None;
    let mut label_rule_reached = false;
    if planned_kind == Kind::Movie {
        match movie::movie_plan(disc, discdb, &matches, &groups, &english, settings) {
            StructureOutcome::Plan(found) => {
                label_rule_reached = found.rule != TitleRule::Database;
                found_movie = Some(found);
            }
            StructureOutcome::PlayAll { group } => {
                label_rule_reached = true;
                let key = groups.get(group).map_or("", |found| found.key.as_str());
                if kind_source == KindSource::Discdb {
                    // 8.2 "TheDiscDB states the kind when it knows the disc": its movie stays a movie, so rule 3.1 does
                    // not apply and every play-all-shaped group is an ordinary rule 3 group (a multi-clip feature whose
                    // clips MakeMKV also lists is one).
                    sentences.push(format!(
                        "8.3 rule 3.1: group {key} is play-all shaped, but TheDiscDB states the disc is a movie (8.2), so \
                         rule 3 runs without 3.1"
                    ));
                    found_movie = Some(movie::by_structure_as_movie(disc, &groups, settings));
                } else {
                    planned_kind = Kind::Tv;
                    kind_source = KindSource::PlayAll;
                    sentences.push(format!("8.3 rule 3.1: group {key} is a play-all title, so the disc is TV (8.5)"));
                }
            }
        }
    }
    let mapped_tv = if planned_kind == Kind::Tv {
        discdb.and_then(|record| tv::by_discdb(record, &matches, &groups, disc.is_dvd))
    } else {
        None
    };

    // 8.1 "Runtime check": the structural pass's durations, before the TMDb match (6.1 step 5's order).
    let runtime = RuntimeFacts {
        feature_seconds: found_movie.as_ref().map_or_else(Vec::new, |found| {
            found.features.iter().filter_map(|feature| disc.titles.get(&feature.title).and_then(|title| title.seconds)).collect()
        }),
        longest_rival_seconds: found_movie.as_ref().and_then(|found| {
            groups.iter().filter(|group| found.rivals.contains(&group.key)).map(|group| group.seconds).max()
        }),
        episode_group_seconds: kind::episode_length_groups(&episode_groups).iter().map(|group| group.seconds).collect(),
    };

    // 8.1 steps 1 to 6, after the owner's Set identity and a remembered identity (8.8: each sets 1.00).
    let mut lookups = inputs.lookups.clone();
    if let Some(remembered) = inputs.remembered {
        lookups.remembered = Some(remembered.tmdb);
    }
    if let Some(reviewed) = inputs.overrides.and_then(|overrides| overrides.identity) {
        lookups.reviewed = Some(reviewed);
    }
    let identity = identify(&lookups, planned_kind, &runtime, settings);
    let record = inputs.lookups.record.filter(|record| identity.tmdb == Some(record.identity));

    // R4: the wanted languages, and 8.3 rule 2 again once the original language is known (Review Focus 6).
    let wanted = wanted_languages(identity.original_language.as_deref());
    if let Some(found) = found_movie.as_mut() {
        if found.rule == TitleRule::Labels && !wanted.original_is_english {
            if let Some(again) = movie::by_labels(disc, &wanted) {
                sentences.push(format!(
                    "8.3 rule 2 chose each cut's variant again with the original language ({})",
                    wanted.original.join(", ")
                ));
                *found = again;
            }
        }
    }

    // 8.3 rule 4, once TMDb's runtime is known.
    let runtime_min = record.filter(|record| record.identity.kind == TmdbKind::Movie).and_then(|record| record.runtime);
    let found_movie = found_movie.map(|found| movie::runtime_sanity(found, runtime_min));

    // 8.5: the map, or the suggestions from the season session's next unassigned episode.
    let season_hint = inputs.overrides.and_then(|overrides| overrides.season).or_else(|| name.and_then(|name| name.season));
    let show = identity.tmdb.filter(|tmdb| tmdb.kind == TmdbKind::Tv);
    let mut planned = match (found_movie, mapped_tv) {
        (Some(found), _) => Planned::Movie(found),
        (None, Some(found)) => Planned::Tv(found),
        (None, None) => {
            let (season, start, found) = match show.map(|show| tv::find(inputs.sessions, show.id, season_hint)) {
                Some(Found::Session(session)) | Some(Found::SingleOpen(session)) => {
                    sentences.push(format!(
                        "season session {} gives season {} and starts the suggestions at episode {}",
                        session.file_name(),
                        session.season,
                        session.next_unassigned()
                    ));
                    (Some(session.season), session.next_unassigned(), Some(session))
                }
                Some(Found::Ambiguous) => {
                    sentences.push("the show has several open season sessions: review asks for the season (8.5)".to_string());
                    (season_hint, 1, None)
                }
                Some(Found::None) | None => (season_hint, 1, None),
            };
            let episodes = record.and_then(|record| season.and_then(|season| record.season(season)));
            Planned::Tv(tv::unmapped(&episode_groups, season, start, episodes, found))
        }
    };
    if let Planned::Tv(found) = &mut planned {
        for item in &mut found.episodes {
            if item.episode.season.is_none() {
                item.episode.season = season_hint;
            }
        }
    }

    let (title_rule, rivals, mut notes) = match &planned {
        Planned::Movie(found) => {
            sentences.extend(found.evidence.iter().cloned());
            (Some(found.rule), found.rivals.clone(), found.notes.clone())
        }
        Planned::Tv(found) => {
            sentences.extend(found.evidence.iter().cloned());
            (None, Vec::new(), Vec::new())
        }
    };
    if label_rule_reached && !disc.java_in_use && !disc.is_dvd {
        notes.push(Noted { note: Note::LabelRuleUnavailable, stream: None, detail: LABEL_RULE_UNAVAILABLE.to_string() });
    }

    // The titles in plan order (which fixes the keys): assigned titles (spec 5, 11) replace the planned ones at 1.00.
    let assigned = assigned_roles(inputs, &mut sentences);
    let (entries, mut title_confidence, mut title_reviews) = match &assigned {
        Some(roles) => {
            let (assigned_kind, entries) = assigned_entries(roles, disc, inputs.library, planned_kind, &mut sentences);
            if assigned_kind != planned_kind {
                sentences.push(format!("the assigned titles make the disc {}", kind_words(assigned_kind)));
            }
            planned_kind = assigned_kind;
            (entries, REMEMBERED_CONFIDENCE, Vec::new())
        }
        None => match &planned {
            Planned::Movie(found) => (movie_entries(found, inputs.library), found.confidence, found.reviews.clone()),
            Planned::Tv(found) => (tv_entries(found), found.confidence, found.reviews.clone()),
        },
    };
    let titles = planned_titles(disc, entries);

    // The season session (8.5 "A disc joins a session when its identity is confirmed (by TheDiscDB or in review)"): a
    // mapped TV disc whose identity is confirmed (TheDiscDB, a remembered decision or the owner's Set identity; a search
    // identity is not), or episodes assigned in review, join it with their episodes; an unmapped TV disc whose identity
    // is confirmed is attached to it with no episode (spec 11 Set identity "attaches the disc to a season session").
    let confirmed = matches!(
        identity.step,
        IdentityStep::Reviewed | IdentityStep::Remembered | IdentityStep::DiscdbBoth | IdentityStep::DiscdbOne
    );
    let (joins, attaches) = if planned_kind != Kind::Tv {
        (false, false)
    } else {
        match &assigned {
            Some(_) => (titles.iter().any(|planned| matches!(planned.role, PlannedRole::Episode(_))), false),
            None => match &planned {
                Planned::Tv(found) if found.mapped => (confirmed, false),
                Planned::Tv(_) => (false, confirmed),
                Planned::Movie(_) => (false, false),
            },
        }
    };
    let session = match show {
        Some(show) if joins || attaches => {
            join_session(inputs.sessions, show, record, inputs.fingerprint, &titles, joins, &mut sentences)
        }
        _ => None,
    };

    // Not settled by the spec (left to the owner): a search identity of the other kind than the planned disc holds it
    // for review (D1), since its runtime check was its own kind's (8.1). The planned titles may then be the wrong ones
    // (a TV disc planned as a movie keeps one feature), so they are held at 0.60 with title_ambiguous and the disc is
    // copied whole (6.3, 8.8 "Limits of D1": review never needs the disc again); assigned titles stay at 1.00.
    let kind_review = kind_disagreement(&identity, planned_kind, &mut sentences);
    if kind_review.is_some() && assigned.is_none() {
        title_confidence = title_confidence.min(movie::AMBIGUOUS_TITLE_CONFIDENCE);
        title_reviews.push(ReviewReason::TitleAmbiguous);
        sentences.push(format!(
            "the kind contradiction holds the titles at {title_confidence:.2} with title_ambiguous, for whole-disc copy \
             mode (6.3, 8.8 Limits of D1)"
        ));
    }

    // 8.8: identity and title confidence are fixed here; the mode follows the title confidence (6.2, 6.3).
    let reviews = sorted_unique(identity.review.into_iter().chain(title_reviews).chain(kind_review).collect());
    let planned_mode = mode(title_confidence, settings);
    sentences.push(mode_sentence(title_confidence, planned_mode));
    let identity_confidence = identity.confidence;
    let labels = label_evidence(disc);
    Ok(Plan {
        identity,
        kind: planned_kind,
        kind_source,
        titles,
        mode: planned_mode,
        identity_confidence,
        title_confidence,
        reviews,
        notes,
        wanted,
        session,
        evidence: PlanEvidence {
            name: name.cloned(),
            candidates,
            groups,
            rivals,
            title_rule,
            labels,
            discdb_matches: matches.iter().map(|(&discdb_index, &title)| DiscDbMatch { discdb_index, title }).collect(),
            sentences,
        },
    })
}

/// The roles that replace the planned titles: the owner's Assign titles when it names any title, else the remembered
/// decision's titles the scan matches (`remembered::apply_titles`) when it matches every one of them that is not
/// Ignored; None when neither applies. Spec 5, 11.
fn assigned_roles(inputs: &PlanInputs<'_>, sentences: &mut Vec<String>) -> Option<BTreeMap<u32, TitleRole>> {
    if let Some(overrides) = inputs.overrides.filter(|overrides| !overrides.titles.is_empty()) {
        sentences.push("the titles are the owner's Assign titles (spec 11): title confidence 1.00".to_string());
        return Some(overrides.titles.iter().map(|entry| (entry.title, entry.role.clone())).collect());
    }
    let remembered = inputs.remembered?;
    let roles = crate::remembered::apply_titles(remembered, inputs.disc);
    if roles.is_empty() {
        sentences.push("the remembered decision matches no title of this scan: the titles are planned".to_string());
        return None;
    }
    // Spec 5 "A remembered decision sets the confidence it covers to 1.00": one that does not match every title it keeps
    // does not cover this scan (taken whole it would leave the unmatched titles out of a Direct job, 8.8 "Limits of D1").
    let kept = remembered.titles.iter().filter(|title| !matches!(title.role, TitleRole::Ignored)).count();
    let matched = roles.values().filter(|role| !matches!(role, TitleRole::Ignored)).count();
    if matched < kept {
        sentences.push(format!(
            "the remembered decision matches only {matched} of its {kept} titles: the titles are planned"
        ));
        return None;
    }
    sentences.push(format!(
        "the titles are the remembered decision's ({} titles matched, spec 5): title confidence 1.00",
        roles.len()
    ));
    Some(roles.into_iter().map(|(number, role)| (number, role.clone())).collect())
}

/// Spec 11's Assign titles as a feature for 8.4's `edition_names`: `Feature` is the theatrical or main cut (no edition),
/// `Cut { edition }` a feature with the owner's name, theatrical when `movie::is_theatrical` says the name names the
/// theatrical cut (Task 5's one test of "theatrical"); None for episodes and Ignored. The one copy of this mapping: the
/// re-plan's `assigned_entries` and the decision (`decision::decide` and its `job_features`) both call it, so a job
/// approved at the disposition gate is filed as a re-plan would plan it (Open point 32). Spec 11, 8.4. Def: T13.
/// Use: T13 (`decision::decide`, `job_features`, and Task 7's `assigned_entries` as Task 13 rewrites it).
pub(crate) fn assigned_feature(number: u32, role: &TitleRole) -> Option<Feature> {
    match role {
        TitleRole::Feature => Some(Feature { title: number, edition: None, theatrical: true, siblings: Vec::new() }),
        TitleRole::Cut { edition } => Some(Feature {
            title: number,
            edition: Some(edition.clone()),
            theatrical: is_theatrical(Some(edition.as_str())),
            siblings: Vec::new(),
        }),
        TitleRole::Episodes { .. } | TitleRole::Ignored => None,
    }
}

/// The assigned titles as plan entries and the kind they make: features and cuts in title order with 8.4's edition names
/// (a Cut keeps the owner's name; a Feature is the theatrical or main cut), then episodes by (season, first episode); an
/// Ignored title or a title the scan lacks is left out. Only episodes make the disc TV, only features a movie; a mix, or
/// nothing, keeps `planned_kind`.
fn assigned_entries(
    roles: &BTreeMap<u32, TitleRole>,
    disc: &Disc,
    library: &LibraryFacts,
    planned_kind: Kind,
    sentences: &mut Vec<String>,
) -> (Kind, Vec<Entry>) {
    let mut features = Vec::new();
    let mut episodes: Vec<(u32, Episode)> = Vec::new();
    for (&number, role) in roles {
        if !disc.titles.contains_key(&number) {
            sentences.push(format!("assigned title {number} is not in the scan and is left out"));
            continue;
        }
        match role {
            TitleRole::Feature | TitleRole::Cut { .. } => features.extend(assigned_feature(number, role)),
            TitleRole::Episodes { season, episodes: numbers } => episodes.push((
                number,
                Episode { season: Some(*season), episodes: numbers.clone(), name: None },
            )),
            TitleRole::Ignored => sentences.push(format!("title {number} is ignored")),
        }
    }
    let kind = match (features.is_empty(), episodes.is_empty()) {
        (false, true) => Kind::Movie,
        (true, false) => Kind::Tv,
        _ => planned_kind,
    };
    let editions = movie::edition_names(&features, library);
    let mut entries: Vec<Entry> = features
        .iter()
        .zip(editions)
        .map(|(feature, edition)| Entry {
            title: feature.title,
            role: PlannedRole::Feature { edition },
            suggested: false,
            remembered: true,
        })
        .collect();
    episodes.sort_by_key(|(title, episode)| episode_order(*title, episode));
    entries.extend(episodes.into_iter().map(|(title, episode)| Entry {
        title,
        role: PlannedRole::Episode(episode),
        suggested: false,
        remembered: true,
    }));
    (kind, entries)
}

/// `plan`'s refusal: a scan with no title.
const NO_TITLE: &str = "the scan lists no title, so there is nothing to plan";

/// The detail of the LabelRuleUnavailable note (8.3 rule 2; 10.2 ToolMissing: "a note records why").
const LABEL_RULE_UNAVAILABLE: &str =
    "MakeMKV did not report a Java runtime in use (no MSG:3344), so label rule 8.3.2 does not apply";

/// What the title rules planned (8.3 or 8.5), before any remembered or review decision.
enum Planned {
    Movie(MoviePlan),
    Tv(TvPlan),
}

/// A planned title before its key is fixed.
struct Entry {
    title: u32,
    role: PlannedRole,
    suggested: bool,
    remembered: bool,
}

/// The movie titles in feature order (database index order for rule 1, each cut's lowest title number for rule 2, group
/// order for rule 3) with 8.4's edition names over the library facts. Two or more structural rivals carry no edition
/// name: rule 3 names no cut, and such a plan is held for review, where the owner names the cuts (8.4).
fn movie_entries(found: &MoviePlan, library: &LibraryFacts) -> Vec<Entry> {
    let editions = if found.rule == TitleRule::Structure && found.features.len() >= 2 {
        vec![None; found.features.len()]
    } else {
        movie::edition_names(&found.features, library)
    };
    found
        .features
        .iter()
        .zip(editions)
        .map(|(feature, edition)| Entry {
            title: feature.title,
            role: PlannedRole::Feature { edition },
            suggested: false,
            remembered: false,
        })
        .collect()
}

/// The episode titles ordered by (season, first episode), those without a season last in title order.
fn tv_entries(found: &TvPlan) -> Vec<Entry> {
    let mut episodes: Vec<&EpisodeTitle> = found.episodes.iter().collect();
    episodes.sort_by_key(|item| episode_order(item.title, &item.episode));
    episodes
        .into_iter()
        .map(|item| Entry {
            title: item.title,
            role: PlannedRole::Episode(item.episode.clone()),
            suggested: item.suggested,
            remembered: false,
        })
        .collect()
}

/// The sort key of an episode title: (season unknown, season, first episode, title number).
fn episode_order(title: u32, episode: &Episode) -> (bool, u32, u32, u32) {
    match episode.season {
        Some(season) => (false, season, episode.episodes.first().copied().unwrap_or(u32::MAX), title),
        None => (true, 0, 0, title),
    }
}

/// The planned titles with their keys (spec 5: the key's NN is the position in the plan) and the scan's facts.
fn planned_titles(disc: &Disc, entries: Vec<Entry>) -> Vec<PlannedTitle> {
    let mut titles = Vec::with_capacity(entries.len());
    for entry in entries {
        if let Some(title) = disc.titles.get(&entry.title) {
            titles.push(PlannedTitle {
                key: title_key(titles.len(), title, disc.is_dvd),
                title: entry.title,
                role: entry.role,
                seconds: title.seconds,
                size: title.size,
                source_file: title.source_file.clone(),
                suggested: entry.suggested,
                remembered: entry.remembered,
            });
        }
    }
    titles
}

/// Spec 8.5 "A disc joins a session when its identity is confirmed": the show's session for the season of the first
/// planned episode that has one (`tv::find`), else a new one with the record's episode count for that season. With
/// `with_episodes` the planned episodes of that season are assigned under the fingerprint; without, the disc is only
/// recorded (spec 11 Set identity "attaches the disc to a season session": `tv::assign` with no pair). Nothing is
/// recorded without a fingerprint. None when no planned episode has a season.
fn join_session(
    sessions: &[SeasonSession],
    show: TmdbIdentity,
    record: Option<&TmdbRecord>,
    fingerprint: Option<&str>,
    titles: &[PlannedTitle],
    with_episodes: bool,
    sentences: &mut Vec<String>,
) -> Option<SeasonSession> {
    let episodes: Vec<(u32, &Episode)> = titles
        .iter()
        .filter_map(|planned| match &planned.role {
            PlannedRole::Episode(episode) => Some((planned.title, episode)),
            PlannedRole::Feature { .. } => None,
        })
        .collect();
    let season = episodes.iter().find_map(|(_, episode)| episode.season)?;
    let mut session = match tv::find(sessions, show.id, Some(season)) {
        Found::Session(found) => found.clone(),
        Found::SingleOpen(_) | Found::None | Found::Ambiguous => {
            let count = record.and_then(|record| record.season(season)).and_then(|listed| listed.episode_count);
            SeasonSession::new(show.id, season, count)
        }
    };
    if let Some(fingerprint) = fingerprint {
        let pairs: Vec<(u32, u32)> = if with_episodes {
            episodes
                .iter()
                .filter(|(_, episode)| episode.season == Some(season))
                .flat_map(|(title, episode)| episode.episodes.iter().map(move |number| (*number, *title)))
                .collect()
        } else {
            Vec::new()
        };
        tv::assign(&mut session, fingerprint, &pairs);
        sentences.push(if with_episodes {
            format!("the disc joins season session {} with {} episodes", session.file_name(), pairs.len())
        } else {
            format!("the disc is attached to season session {} until review assigns its episodes", session.file_name())
        });
    }
    Some(session)
}

/// Every title comment that is an 8.3 rule 2 label, in title order.
fn label_evidence(disc: &Disc) -> Vec<(u32, String)> {
    disc.titles
        .values()
        .filter_map(|title| {
            let comment = title.comment.as_deref()?;
            movie::parse_label(title.number, comment).map(|_| (title.number, comment.to_string()))
        })
        .collect()
}

/// The fingerprint and the disc ID the plan was made for, as sentences.
fn input_sentences(inputs: &PlanInputs<'_>) -> Vec<String> {
    let mut sentences = Vec::new();
    if let Some(fingerprint) = inputs.fingerprint {
        sentences.push(format!("fingerprint {fingerprint}"));
    }
    if let Some(disc_id) = inputs.disc_id {
        sentences.push(format!("disc ID {disc_id}"));
    }
    sentences
}

/// Which TheDiscDB record the titles were matched against, and how many matched; without a record, whether TheDiscDB
/// gave none or its lookup is pending (8.1 "the lookup is recorded as pending").
fn discdb_sentence(lookups: &LookupInputs<'_>, matched: usize) -> String {
    match (lookups.discdb_by_fingerprint, lookups.discdb_by_disc_id) {
        (Some(_), _) => format!("TheDiscDB's record by fingerprint is matched against the scan: {matched} titles match"),
        (None, Some(_)) => format!("TheDiscDB's record by disc ID is matched against the scan: {matched} titles match"),
        (None, None) if lookups.discdb_pending => {
            "the TheDiscDB lookup is pending: the titles are planned from the scan".to_string()
        }
        (None, None) => "TheDiscDB gave no record: the titles are planned from the scan".to_string(),
    }
}

/// The kind in a sentence.
fn kind_words(kind: Kind) -> &'static str {
    match kind {
        Kind::Movie => "a movie",
        Kind::Tv => "TV",
    }
}

/// Why the kind is what it is (8.2, 8.3 rule 3.1).
fn kind_source_words(source: KindSource) -> &'static str {
    match source {
        KindSource::Discdb => "TheDiscDB states the kind",
        KindSource::Structure => "three or more episode-length groups within 15% of each other",
        KindSource::SeasonMarker => "the disc's name has a season marker",
        KindSource::PlayAll => "a play-all title (8.3 rule 3.1)",
        KindSource::Default => "no TV evidence",
    }
}

/// `identity_ambiguous`, with a sentence, when a search identity (8.1 steps 4 and 5) is of the other kind than the
/// planned disc; None otherwise. The spec does not settle this case (left to the owner): the search's runtime check was
/// the result's own kind's, so nothing else would hold such a disc (D1). `plan` then also holds the planned titles at
/// 0.60 with title_ambiguous (whole-disc copy mode, 6.3, 8.8 "Limits of D1") unless assigned titles cover them; this
/// provisional rule is flagged for the owner with the first. Several plausible results (TmdbSeveral) of which
/// `identify` picked none (it narrows a mixed-kind list to the planned kind and picks none when every result is of the
/// other kind) contradict the plan the same way when every plausible result is of the other kind; a mixed-kind list does
/// not (its pick is of the planned kind, and 8.3 rule 4 remains the guard). A TheDiscDB, remembered or reviewed
/// identity is not checked: TheDiscDB states the kind (8.2), and the owner chose the other two.
fn kind_disagreement(identity: &Identity, planned_kind: Kind, sentences: &mut Vec<String>) -> Option<ReviewReason> {
    let kind_of = |tmdb: &TmdbIdentity| match tmdb.kind {
        TmdbKind::Movie => Kind::Movie,
        TmdbKind::Tv => Kind::Tv,
    };
    let other = match planned_kind {
        Kind::Movie => Kind::Tv,
        Kind::Tv => Kind::Movie,
    };
    let sentence = match (identity.step, identity.tmdb.as_ref()) {
        (IdentityStep::TmdbOne | IdentityStep::TmdbSeveral, Some(tmdb)) if kind_of(tmdb) == other => {
            format!("the TMDb identity is {} but the disc is planned as {}", kind_words(other), kind_words(planned_kind))
        }
        (IdentityStep::TmdbSeveral, None)
            if !identity.plausible.is_empty() && identity.plausible.iter().all(|tmdb| kind_of(tmdb) == other) =>
        {
            format!(
                "every plausible TMDb result is {} but the disc is planned as {}",
                kind_words(other),
                kind_words(planned_kind)
            )
        }
        _ => return None,
    };
    sentences.push(sentence);
    Some(ReviewReason::IdentityAmbiguous)
}

/// The mode the title confidence sets, as a sentence (6.2, 6.3, 8.8).
fn mode_sentence(title_confidence: f64, planned: Mode) -> String {
    let mode = match planned {
        Mode::Direct => "Direct mode (6.2)",
        Mode::Copy => "whole-disc copy mode (6.3)",
    };
    format!("title confidence {title_confidence:.2}: {mode}")
}

/// Spec 7 "Space checks": 5% over the planned titles' sizes before a Direct rip. Def: T7. Use: phase 2.
pub const DIRECT_SPACE_MARGIN: f64 = 0.05;
/// Spec 7 "Space checks": 2% over the disc size before a whole-disc copy. Def: T7. Use: phase 2.
pub const COPY_SPACE_MARGIN: f64 = 0.02;
/// Spec 7 "Space checks": 5% over the files to be written before extraction or remux. Def: T7. Use: phase 2.
pub const WRITE_SPACE_MARGIN: f64 = 0.05;

/// Bytes needed on staging for the plan's mode: Direct, the sum of `PlannedTitle.size` (a title without a size counts 0)
/// times 1.05; Copy, `disc_size` times 1.02; rounded up. When staging and a library share a volume the engine adds the
/// needs (spec 7); that is phase 2's. Def: T7. Use: phase 2.
pub fn staging_bytes_needed(plan: &Plan, disc_size: u64) -> u64 {
    match plan.mode {
        Mode::Direct => {
            let planned = plan.titles.iter().map(|title| title.size.unwrap_or(0)).fold(0u64, u64::saturating_add);
            with_margin(planned, DIRECT_SPACE_MARGIN)
        }
        Mode::Copy => with_margin(disc_size, COPY_SPACE_MARGIN),
    }
}

/// `bytes` times 1.05, rounded up (before extraction or remux). Def: T7. Use: phase 2.
pub fn write_bytes_needed(bytes: u64) -> u64 {
    with_margin(bytes, WRITE_SPACE_MARGIN)
}

/// `bytes` plus `margin` (a whole percentage: 0.05 is 5%), rounded up to the next byte in integer arithmetic so no
/// size loses a byte to floating point; at most u64::MAX.
fn with_margin(bytes: u64, margin: f64) -> u64 {
    let percent = (margin * 100.0).round() as u128;
    let needed = (u128::from(bytes) * (100 + percent)).div_ceil(100);
    u64::try_from(needed).unwrap_or(u64::MAX)
}

/// The inputs the tests of `plan::kind`, `plan::movie` and this file share: the nine pinned TheDiscDB logs, their
/// live answers, and the streams synthetic scans are built from. Test builds only; nothing here reads the evidence
/// clone.
#[cfg(test)]
pub(crate) mod fixtures {
    use std::fs;
    use std::path::Path;

    use crate::disc::{disc, Disc};
    use crate::lookup::discdb::{parse_dischash, DiscDbRecord};
    use crate::robot::parse;

    pub(crate) const JOHN_WICK_UHD: &str = "thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt";
    pub(crate) const JOHN_WICK_BLU_RAY: &str = "thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc02.txt";
    pub(crate) const KNOWING: &str = "thediscdb/movie/Knowing (2009)/2010-dvd/disc01.txt";
    pub(crate) const SMILE: &str = "thediscdb/movie/Smile (2022)/2022-4k/disc01.txt";
    pub(crate) const EXPENDABLES: &str =
        "thediscdb/movie/The Expendables 3 (2014)/the-expendables-3-film-collection/disc01.txt";
    pub(crate) const CHERNOBYL_1: &str = "thediscdb/series/Chernobyl (2019)/2020-4k-US/disc01.txt";
    pub(crate) const CHERNOBYL_1_PINNED_JSON: &str = "thediscdb/series/Chernobyl (2019)/2020-4k-US/disc01.json";
    pub(crate) const CHERNOBYL_2: &str = "thediscdb/series/Chernobyl (2019)/2020-4k-US/disc02.txt";
    pub(crate) const FIREFLY_1: &str = "thediscdb/series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt";
    pub(crate) const FIREFLY_2: &str = "thediscdb/series/Firefly (2002)/2003-dvd-the-complete-series/disc02.txt";

    /// The `dischash` answers of `corpus/thediscdb-api` for the logs above (their `ContentHash`).
    pub(crate) const JOHN_WICK_UHD_API: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
    pub(crate) const EXPENDABLES_API: &str = "545411B47060B18FD7F8D94599F24708";
    pub(crate) const SMILE_API: &str = "E826F0C64B04079EA83094AC046E8709";
    pub(crate) const KNOWING_API: &str = "631BD77872C157D7C436E6F48F06A478";
    pub(crate) const CHERNOBYL_1_API: &str = "D11592C9B2D252937C22EDF01DBD1E05";
    pub(crate) const CHERNOBYL_2_API: &str = "59796E40C7ABE16ACD9B259E732D392B";
    pub(crate) const FIREFLY_1_API: &str = "5A961BC67EB34171F24852D3A7688020";
    pub(crate) const JOHN_WICK_BLU_RAY_API: &str = "BB9E6C963A4E2EAAB46C748CD4C00B69";
    pub(crate) const FIREFLY_2_API: &str = "294A3E64C457A1BC9A0833335C090568";

    /// CINFO 1 of a Blu-ray and of a DVD.
    pub(crate) const BLURAY: &[(u32, &str)] = &[(1, "Blu-ray disc")];
    pub(crate) const DVD: &[(u32, &str)] = &[(1, "DVD disc")];
    /// Streams as MakeMKV reports them (attributes 1, 3, 5, 6, 7, 14), for synthetic titles.
    pub(crate) const VIDEO: &[(u32, &str)] =
        &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")];
    pub(crate) const DVD_VIDEO: &[(u32, &str)] = &[(1, "Video"), (5, "V_MPEG2"), (6, "Mpeg2"), (7, "Mpeg2")];
    pub(crate) const ENG_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];
    pub(crate) const SPA_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "spa"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];
    pub(crate) const FRA_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "fra"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];
    pub(crate) const JPN_AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "jpn"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")];
    pub(crate) const ENG_PGS: &[(u32, &str)] =
        &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];

    /// A file under `corpus/`, read whole (the logs are UTF-8; any stray byte is replaced, as the parser's callers do).
    pub(crate) fn corpus_text(relative: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(relative);
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The typed disc of a robot log's text.
    pub(crate) fn disc_of(text: &str) -> Disc {
        disc(&parse(text)).expect("the scan builds a typed disc")
    }

    /// The typed disc of one pinned log.
    pub(crate) fn pinned(relative: &str) -> Disc {
        disc_of(&corpus_text(relative))
    }

    /// The `dischash` answer `corpus/thediscdb-api/dischash-<hash>.json`, which must be a hit.
    pub(crate) fn api_record(hash: &str) -> DiscDbRecord {
        let name = format!("thediscdb-api/dischash-{hash}.json");
        parse_dischash(&corpus_text(&name), &name).expect("the answer parses").expect("the answer is a hit")
    }

    /// A pinned `discNN.json` (the data repository's form, without `media`).
    pub(crate) fn pinned_record(relative: &str) -> DiscDbRecord {
        serde_json::from_str(&corpus_text(relative)).expect("the pinned record parses")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disc::Disc;

    #[test]
    fn direct_mode_needs_the_auto_threshold() {
        let settings = Settings::default();
        assert_eq!(mode(0.98, &settings), Mode::Direct);
        assert_eq!(mode(0.85, &settings), Mode::Direct);
        assert_eq!(mode(0.84, &settings), Mode::Copy);
        assert_eq!(mode(0.60, &settings), Mode::Copy);
    }

    #[test]
    fn a_higher_auto_threshold_sends_a_0_85_plan_to_copy_mode() {
        let settings = Settings { auto_threshold: 0.90, ..Settings::default() };
        assert_eq!(mode(0.85, &settings), Mode::Copy);
        assert_eq!(mode(0.90, &settings), Mode::Direct);
        assert_eq!(mode(0.98, &settings), Mode::Direct);
    }

    #[test]
    fn modes_spell_themselves_as_the_spec_does() {
        assert_eq!(Mode::ALL.map(Mode::as_str), ["direct", "copy"]);
        for mode in Mode::ALL {
            assert_eq!(serde_json::to_string(&mode).unwrap(), format!("\"{}\"", mode.as_str()));
            assert_eq!(mode.as_str().parse::<Mode>(), Ok(mode));
        }
        assert!("Direct".parse::<Mode>().is_err());
    }

    #[test]
    fn the_plan_module_reexports_the_kind_types() {
        // Chiefly a compile check: `plan::Group`, `plan::KindSource` and `plan::Kind` must be the kind module's items (the
        // function pointer type-checks only when `plan::Group` is `kind::Group`); the serde spellings are then read through
        // the re-exported paths.
        let _: fn(&Disc, &[u32]) -> Vec<Group> = kind::groups;
        assert_eq!(serde_json::to_string(&KindSource::PlayAll).unwrap(), "\"play_all\"");
        assert_eq!(serde_json::to_string(&Kind::Tv).unwrap(), "\"tv\"");
    }

    use super::fixtures::*;
    use crate::testsupport::scan_text;

    #[test]
    fn title_key_on_a_blu_ray_is_the_plan_position_and_the_source_file_stem() {
        let disc = pinned(JOHN_WICK_UHD);
        let playlist = disc.title(0).unwrap();
        let clip = disc.title(8).unwrap();
        assert_eq!(playlist.source_file.as_deref(), Some("00001.mpls"));
        assert_eq!(clip.source_file.as_deref(), Some("00001.m2ts"));
        assert_eq!(title_key(0, playlist, false), "t00-00001");
        assert_eq!(title_key(1, clip, false), "t01-00001");
        assert_eq!(title_key(12, playlist, false), "t12-00001");
        assert_eq!(title_key(100, playlist, false), "t100-00001");
        let expendables = pinned(EXPENDABLES);
        assert_eq!(title_key(0, expendables.title(41).unwrap(), false), "t00-00756");
    }

    #[test]
    fn title_key_on_a_dvd_is_the_plan_position_and_attribute_24_as_written() {
        let knowing = pinned(KNOWING);
        let title = knowing.title(0).unwrap();
        assert_eq!((title.dvd_title.as_deref(), title.source_file.as_deref()), (Some("01"), None));
        assert_eq!(title_key(0, title, true), "t00-title01");
        // Firefly disc 2's scan title 5 is DVD title 11: the key names the DVD title, not the scan's number.
        let firefly = pinned(FIREFLY_2);
        assert_eq!(title_key(3, firefly.title(5).unwrap(), true), "t03-title11");
    }

    #[test]
    fn a_title_without_its_source_is_keyed_by_its_scan_number() {
        let blu_ray = disc_of(&scan_text(BLURAY, &[(7, &[(9, "0:10:00")], &[(0, VIDEO)])]));
        assert_eq!(title_key(2, blu_ray.title(7).unwrap(), false), "t02-title07");
        let dvd = disc_of(&scan_text(DVD, &[(3, &[(9, "0:10:00")], &[(0, DVD_VIDEO)])]));
        assert_eq!(title_key(0, dvd.title(3).unwrap(), true), "t00-title03");
    }

    use crate::fingerprint::{content_hash, hashed_files};
    use crate::lookup::name::disc_name;
    use crate::lookup::tmdb::{parse_movie, parse_search, parse_tv, TmdbSearch};
    use crate::lookup::IdentityStep;
    use crate::testsupport::{scan_text_with_messages, search_json, tmdb_movie_json, tmdb_tv_json};
    use serde_json::json;
    use std::collections::BTreeMap;

    /// Plan inputs with no fingerprint, no disc ID, nothing remembered or reviewed and no season session.
    fn inputs<'a>(
        disc: &'a Disc,
        lookups: LookupInputs<'a>,
        settings: &'a Settings,
        library: &'a LibraryFacts,
    ) -> PlanInputs<'a> {
        PlanInputs {
            disc,
            fingerprint: None,
            disc_id: None,
            lookups,
            remembered: None,
            overrides: None,
            sessions: &[],
            library,
            settings,
        }
    }

    /// `plan` of inputs whose disc has titles.
    fn run(inputs: &PlanInputs<'_>) -> Plan {
        plan(inputs).expect("the disc has titles")
    }

    /// One pinned log with what `lazymkv-dev plan` reads for it (Open point 26): the typed disc, the fingerprint of its
    /// HSH lines, the disc name from its volume label (a pinned log has no bdmt file), and the `dischash` answer of
    /// `corpus/thediscdb-api` that the fingerprint names.
    struct PinnedLog {
        disc: Disc,
        fingerprint: String,
        record: DiscDbRecord,
        name: Option<DiscName>,
    }

    fn pinned_log(log: &str, hash: &str) -> PinnedLog {
        let text = corpus_text(log);
        let fingerprint = content_hash(&hashed_files(&text)).expect("the log lists the disc's files");
        assert_eq!(fingerprint, hash, "{log}: its fingerprint names its TheDiscDB answer");
        let disc = disc_of(&text);
        let name = disc_name(None, disc.volume_label.as_deref());
        PinnedLog { record: api_record(&fingerprint), disc, fingerprint, name }
    }

    impl PinnedLog {
        /// The lookups `plan_doc` gives a pinned log: the disc name and TheDiscDB by fingerprint, no TMDb answer.
        fn lookups(&self) -> LookupInputs<'_> {
            LookupInputs { name: self.name.as_ref(), discdb_by_fingerprint: Some(&self.record), ..LookupInputs::default() }
        }

        /// The disc name only, as if TheDiscDB did not know the disc.
        fn name_only(&self) -> LookupInputs<'_> {
            LookupInputs { name: self.name.as_ref(), ..LookupInputs::default() }
        }

        /// The plan with the fingerprint, the default settings, no library facts and no season session.
        fn plan(&self) -> Plan {
            let settings = Settings::default();
            let library = LibraryFacts::default();
            let mut planned = inputs(&self.disc, self.lookups(), &settings, &library);
            planned.fingerprint = Some(self.fingerprint.as_str());
            run(&planned)
        }
    }

    /// A planned title as the planner writes it from the scan, neither suggested nor remembered.
    fn planned(key: &str, title: u32, role: PlannedRole, seconds: u32, size: u64, source_file: Option<&str>) -> PlannedTitle {
        PlannedTitle {
            key: key.to_string(),
            title,
            role,
            seconds: Some(seconds),
            size: Some(size),
            source_file: source_file.map(str::to_string),
            suggested: false,
            remembered: false,
        }
    }

    fn feature(edition: Option<&str>) -> PlannedRole {
        PlannedRole::Feature { edition: edition.map(str::to_string) }
    }

    fn episode(season: Option<u32>, episodes: &[u32], name: Option<&str>) -> PlannedRole {
        PlannedRole::Episode(Episode { season, episodes: episodes.to_vec(), name: name.map(str::to_string) })
    }

    fn movie_id(id: i64) -> TmdbIdentity {
        TmdbIdentity { kind: TmdbKind::Movie, id }
    }

    fn show_id(id: i64) -> TmdbIdentity {
        TmdbIdentity { kind: TmdbKind::Tv, id }
    }

    /// 8.1 step 3, which every pinned log reaches: TheDiscDB by fingerprint at 0.95 with its TMDb ID, title and year.
    fn assert_discdb_one(plan: &Plan, tmdb: TmdbIdentity, title: &str, year: i32) {
        assert_eq!(plan.identity.step, IdentityStep::DiscdbOne);
        assert_eq!((plan.identity.confidence, plan.identity_confidence), (0.95, 0.95));
        assert_eq!(plan.identity.tmdb, Some(tmdb));
        assert_eq!((plan.identity.title.as_deref(), plan.identity.year), (Some(title), Some(year)));
        assert_eq!(plan.identity.review, None);
        assert!(!plan.identity.lookup_pending);
    }

    /// One synthetic Blu-ray title: number, duration (attribute 9), chapters (8), playlist (16), segment map (26), size
    /// (11; "" leaves it out), comment (49; "" leaves it out), and its audio streams after one video stream.
    struct Made<'a> {
        number: u32,
        duration: &'a str,
        chapters: &'a str,
        playlist: &'a str,
        segments: &'a str,
        size: &'a str,
        comment: &'a str,
        audio: &'a [&'a [(u32, &'a str)]],
    }

    fn made<'a>(
        number: u32,
        duration: &'a str,
        chapters: &'a str,
        playlist: &'a str,
        segments: &'a str,
        size: &'a str,
        comment: &'a str,
        audio: &'a [&'a [(u32, &'a str)]],
    ) -> Made<'a> {
        Made { number, duration, chapters, playlist, segments, size, comment, audio }
    }

    /// English audio, and English then Spanish audio (Dolby Digital 5.1 each).
    const ENG: &[&[(u32, &str)]] = &[ENG_AC3];
    const ENG_SPA: &[&[(u32, &str)]] = &[ENG_AC3, SPA_AC3];

    /// A Blu-ray scan of `titles` named `label` (CINFO 2 and 32), with MakeMKV 1.18.2's banner and, when `java`, MSG:3344.
    fn made_disc(java: bool, label: &str, titles: &[Made<'_>]) -> Disc {
        let attrs: Vec<Vec<(u32, &str)>> = titles
            .iter()
            .map(|made| {
                let mut list = vec![(8, made.chapters), (9, made.duration), (16, made.playlist), (26, made.segments)];
                if !made.size.is_empty() {
                    list.push((11, made.size));
                }
                if !made.comment.is_empty() {
                    list.push((49, made.comment));
                }
                list
            })
            .collect();
        let streams: Vec<Vec<(u32, &[(u32, &str)])>> = titles
            .iter()
            .map(|made| {
                let mut list: Vec<(u32, &[(u32, &str)])> = vec![(0, VIDEO)];
                for (index, audio) in made.audio.iter().enumerate() {
                    list.push((index as u32 + 1, *audio));
                }
                list
            })
            .collect();
        let rows: Vec<(u32, &[(u32, &str)], &[(u32, &[(u32, &str)])])> = titles
            .iter()
            .zip(attrs.iter().zip(&streams))
            .map(|(made, (attrs, streams))| (made.number, attrs.as_slice(), streams.as_slice()))
            .collect();
        let cinfo = [(1, "Blu-ray disc"), (2, label), (32, label)];
        disc_of(&scan_text_with_messages("1.18.2", java, &cinfo, &rows))
    }

    /// A Java disc with two variants of one cut, 100 minutes each: title 1 `FPL_MainFeature` with English audio, title 2
    /// `FPL_MainFeature_SPA` with English and Spanish audio (the shape of 8.3 rule 2's dub variants).
    fn orphanage() -> Disc {
        made_disc(
            true,
            "THE_ORPHANAGE",
            &[
                made(1, "1:40:00", "16", "00001.mpls", "1", "30000000000", "FPL_MainFeature", ENG),
                made(2, "1:40:00", "16", "00002.mpls", "2", "31000000000", "FPL_MainFeature_SPA", ENG_SPA),
            ],
        )
    }

    /// TMDb's `/search/multi` answer for the orphanage disc's name (a synthetic answer of TMDb's documented shape).
    fn orphanage_search() -> TmdbSearch {
        let text = search_json(&[("movie", 6537, "The Orphanage", "El orfanato", "2007-05-20")]);
        parse_search(&text, "search.json").unwrap()
    }

    /// TMDb's `/movie/6537` answer with this original language and runtime (synthetic, TMDb's documented shape).
    fn orphanage_record(original_language: &str, runtime: Option<u32>) -> TmdbRecord {
        let text = tmdb_movie_json(6537, "The Orphanage", "El orfanato", "2007-05-20", original_language, runtime);
        parse_movie(&text, "movie.json").unwrap()
    }

    /// Lookups with the disc name, a TMDb search and maybe the record fetched for its one plausible result.
    fn searched<'a>(name: Option<&'a DiscName>, search: &'a TmdbSearch, record: Option<&'a TmdbRecord>) -> LookupInputs<'a> {
        LookupInputs { name, search: Some(search), record, ..LookupInputs::default() }
    }

    /// Two rival features without Java, 50 and 47 minutes, with segment maps that share nothing (8.3 rule 3.7).
    fn two_rivals() -> Disc {
        made_disc(
            false,
            "TWO_RIVALS",
            &[
                made(1, "0:50:00", "8", "00001.mpls", "1", "4000000000", "", ENG),
                made(2, "0:47:00", "8", "00002.mpls", "2", "3000000000", "", ENG),
            ],
        )
    }

    /// Three episode-length titles of 22:00, 22:30 and 23:00 (segments 1, 2, 3) and, when asked, the play-all title 4
    /// (segments 1,2,3, 1:07:30).
    fn episodes_disc(label: &str, with_play_all: bool) -> Disc {
        let mut titles = vec![
            made(1, "0:22:00", "5", "00001.mpls", "1", "4000000000", "", ENG),
            made(2, "0:22:30", "5", "00002.mpls", "2", "4100000000", "", ENG),
            made(3, "0:23:00", "5", "00003.mpls", "3", "4200000000", "", ENG),
        ];
        if with_play_all {
            titles.push(made(4, "1:07:30", "15", "00004.mpls", "1,2,3", "12300000000", "", ENG));
        }
        made_disc(false, label, &titles)
    }

    /// The episodes disc's planned title `title` (1 to 3) as an 8.5 suggestion.
    fn suggested(key: &str, title: u32, season: Option<u32>, episodes: &[u32], name: Option<&str>) -> PlannedTitle {
        let (seconds, size) = match title {
            1 => (1320, 4_000_000_000),
            2 => (1350, 4_100_000_000),
            _ => (1380, 4_200_000_000),
        };
        let playlist = format!("{title:05}.mpls");
        PlannedTitle {
            suggested: true,
            ..planned(key, title, episode(season, episodes, name), seconds, size, Some(playlist.as_str()))
        }
    }

    /// TMDb's `/search/multi` answer for the episodes disc's cleaned name "SHOW" (synthetic, TMDb's shape).
    fn show_search() -> TmdbSearch {
        parse_search(&search_json(&[("tv", 99, "Show", "Show", "2010-01-01")]), "search.json").unwrap()
    }

    /// TMDb's `/tv/99` with one season expanded: six episodes of 22 or 23 minutes (synthetic, TMDb's shape).
    fn show_record(season: u32) -> TmdbRecord {
        let episodes: &[(u32, &str, Option<u32>)] = &[
            (1, "One", Some(22)),
            (2, "Two", Some(22)),
            (3, "Three", Some(23)),
            (4, "Four", Some(22)),
            (5, "Five", Some(23)),
            (6, "Six", Some(22)),
        ];
        parse_tv(&tmdb_tv_json(99, "Show", "2010-01-01", "en", &[(season, episodes)]), "tv.json").unwrap()
    }

    /// Episode `episode` of a season session, assigned from scan title `title` of the disc `fingerprint`.
    fn assigned_episode(episode: u32, fingerprint: &str, title: u32) -> tv::AssignedEpisode {
        tv::AssignedEpisode { episode, fingerprint: fingerprint.to_string(), title }
    }

    /// A season session with these (episode, title) pairs assigned from `fingerprint`.
    fn session_with(show: i64, season: u32, count: Option<u32>, fingerprint: &str, pairs: &[(u32, u32)]) -> SeasonSession {
        let mut session = SeasonSession::new(show, season, count);
        tv::assign(&mut session, fingerprint, pairs);
        session
    }

    #[test]
    fn john_wick_uhd_plans_its_main_movie_playlist_not_its_clip() {
        let plan = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API).plan();
        assert_discdb_one(&plan, movie_id(245891), "John Wick", 2014);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        assert_eq!(plan.titles, vec![planned("t00-00001", 0, feature(None), 6073, 67_757_930_496, Some("00001.mpls"))]);
        // Title 8 is the playlist's clip (00001.m2ts): the same segment map and duration, but not TheDiscDB's main movie.
        assert!(plan.evidence.groups.iter().any(|group| group.titles == vec![0, 8]));
        assert!(plan.evidence.discdb_matches.contains(&DiscDbMatch { discdb_index: 0, title: 0 }));
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(plan.notes, Vec::<Noted>::new());
        assert_eq!(plan.wanted, wanted_languages(None));
        assert_eq!(plan.session, None);
    }

    #[test]
    fn john_wick_blu_ray_plans_title_0() {
        let plan = pinned_log(JOHN_WICK_BLU_RAY, JOHN_WICK_BLU_RAY_API).plan();
        assert_discdb_one(&plan, movie_id(245891), "John Wick", 2014);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        assert_eq!(plan.titles, vec![planned("t00-00001", 0, feature(None), 6073, 25_234_495_488, Some("00001.mpls"))]);
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(plan.notes, Vec::<Noted>::new());
        assert_eq!(plan.session, None);
    }

    #[test]
    fn expendables_3_plans_its_main_movie_as_the_unrated_cut() {
        let plan = pinned_log(EXPENDABLES, EXPENDABLES_API).plan();
        assert_discdb_one(&plan, movie_id(138103), "The Expendables 3", 2014);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        // Open point 5: TheDiscDB's item title names no edition, so the label table names it (FPL_MainFeature_UR).
        assert_eq!(
            plan.titles,
            vec![planned("t00-00756", 41, feature(Some("Unrated Cut")), 7891, 28_831_358_976, Some("00756.mpls"))]
        );
        assert_eq!(
            plan.evidence.labels,
            vec![(41, "FPL_MainFeature_UR".to_string()), (54, "FPL_MainFeature_R".to_string())]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        // MakeMKV reported Java (MSG:3344), and rule 1 chose anyway: no label-rule note.
        assert_eq!(plan.notes, Vec::<Noted>::new());
    }

    #[test]
    fn smile_plans_its_main_movie_without_an_edition_name() {
        let plan = pinned_log(SMILE, SMILE_API).plan();
        assert_discdb_one(&plan, movie_id(882598), "Smile", 2022);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        // Its label FPL_MainFeature reads "Theatrical Cut", which 8.4 leaves off a film's only cut.
        assert_eq!(plan.titles, vec![planned("t00-00040", 14, feature(None), 6937, 79_673_339_904, Some("00040.mpls"))]);
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(plan.notes, Vec::<Noted>::new());
    }

    #[test]
    fn knowing_dvd_plans_title_0_keyed_by_attribute_24() {
        let plan = pinned_log(KNOWING, KNOWING_API).plan();
        assert_discdb_one(&plan, movie_id(13811), "Knowing", 2009);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        assert_eq!(plan.titles, vec![planned("t00-title01", 0, feature(None), 7274, 7_098_400_768, None)]);
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(plan.notes, Vec::<Noted>::new());
    }

    #[test]
    fn chernobyl_disc_1_plans_three_episodes_in_episode_order_and_starts_a_session() {
        let log = pinned_log(CHERNOBYL_1, CHERNOBYL_1_API);
        let plan = log.plan();
        assert_discdb_one(&plan, show_id(87108), "Chernobyl", 2019);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
        // TheDiscDB lists E01, E03, E02 (index order); the plan files them in episode order. Play-all title 6 is dropped.
        assert_eq!(
            plan.titles,
            vec![
                planned("t00-00801", 1, episode(Some(1), &[1], Some("1.23.45")), 3534, 28_652_482_560, Some("00801.mpls")),
                planned("t01-00802", 8, episode(Some(1), &[2], Some("Please Remain Calm")), 3896, 31_796_779_008, Some("00802.mpls")),
                planned("t02-00803", 7, episode(Some(1), &[3], Some("Open Wide O Earth")), 3707, 30_230_642_688, Some("00803.mpls")),
            ]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(plan.evidence.title_rule, None);
        let name = plan.evidence.name.as_ref().expect("the volume label names the disc");
        assert_eq!((name.raw.as_str(), name.disc, name.season), ("CHERNOBYL_D1", Some(1), None));
        let fingerprint = log.fingerprint.clone();
        assert_eq!(
            plan.session,
            Some(SeasonSession {
                show_id: 87108,
                season: 1,
                discs: vec![fingerprint.clone()],
                assigned: vec![
                    assigned_episode(1, &fingerprint, 1),
                    assigned_episode(2, &fingerprint, 8),
                    assigned_episode(3, &fingerprint, 7),
                ],
                episode_count: None,
                closed: false,
            })
        );
    }

    #[test]
    fn chernobyl_disc_2_plans_episodes_4_and_5() {
        let log = pinned_log(CHERNOBYL_2, CHERNOBYL_2_API);
        let plan = log.plan();
        assert_discdb_one(&plan, show_id(87108), "Chernobyl", 2019);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
        assert_eq!(
            plan.titles,
            vec![
                planned("t00-00801", 6, episode(Some(1), &[4], Some("The Happiness of All Mankind")), 3899, 28_075_038_720, Some("00801.mpls")),
                planned("t01-00802", 8, episode(Some(1), &[5], Some("Vichnaya Pamyat")), 4293, 31_053_785_088, Some("00802.mpls")),
            ]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        let session = plan.session.expect("a mapped disc with a show identity joins its season");
        assert_eq!(
            session.assigned,
            vec![assigned_episode(4, &log.fingerprint, 6), assigned_episode(5, &log.fingerprint, 8)]
        );
    }

    #[test]
    fn firefly_disc_1_plans_three_episodes_by_dvd_title_number() {
        let log = pinned_log(FIREFLY_1, FIREFLY_1_API);
        let plan = log.plan();
        assert_discdb_one(&plan, show_id(1437), "Firefly", 2002);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
        assert_eq!(
            plan.titles,
            vec![
                planned("t00-title01", 0, episode(Some(1), &[1], Some("Serenity")), 5186, 3_971_414_016, None),
                planned("t01-title02", 1, episode(Some(1), &[2], Some("The Train Job")), 2554, 1_956_683_776, None),
                planned("t02-title03", 2, episode(Some(1), &[3], Some("Bushwhacked")), 2627, 1_925_113_856, None),
            ]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        let session = plan.session.expect("a mapped disc with a show identity joins its season");
        assert_eq!((session.show_id, session.season, session.file_name()), (1437, 1, "1437-s01.json".to_string()));
        assert_eq!(
            session.assigned.iter().map(|entry| entry.episode).collect::<Vec<u32>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn firefly_disc_2_plans_episodes_4_to_7() {
        let plan = pinned_log(FIREFLY_2, FIREFLY_2_API).plan();
        assert_discdb_one(&plan, show_id(1437), "Firefly", 2002);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
        // Scan title 5 is DVD title 11 (Review Focus 1): its key names attribute 24.
        assert_eq!(
            plan.titles,
            vec![
                planned("t00-title01", 0, episode(Some(1), &[4], Some("Shindig")), 2631, 1_987_276_800, None),
                planned("t01-title02", 1, episode(Some(1), &[5], Some("Safe")), 2559, 1_906_104_320, None),
                planned("t02-title03", 2, episode(Some(1), &[6], Some("Our Mrs. Reynolds")), 2629, 2_013_861_888, None),
                planned("t03-title11", 5, episode(Some(1), &[7], Some("Jaynestown")), 2628, 1_966_837_760, None),
            ]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
    }

    #[test]
    fn without_thediscdb_a_blu_ray_without_java_is_planned_by_structure_with_the_label_rule_note() {
        let log = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&log.disc, log.name_only(), &settings, &library));
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Nothing, 0.0));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
        // 8.3 rule 3: one group of feature length, `1` (titles 0 and 8); title 0 has the chapters (rule 3.4).
        assert_eq!(plan.titles, vec![planned("t00-00001", 0, feature(None), 6073, 67_757_930_496, Some("00001.mpls"))]);
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
        assert_eq!(plan.evidence.rivals, vec!["1".to_string()]);
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown]);
        assert_eq!(
            plan.notes,
            vec![Noted { note: Note::LabelRuleUnavailable, stream: None, detail: LABEL_RULE_UNAVAILABLE.to_string() }]
        );
    }

    #[test]
    fn without_thediscdb_a_dvd_is_planned_by_structure_without_the_label_rule_note() {
        let log = pinned_log(KNOWING, KNOWING_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&log.disc, log.name_only(), &settings, &library));
        assert_eq!(plan.titles, vec![planned("t00-title01", 0, feature(None), 7274, 7_098_400_768, None)]);
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown]);
        // A DVD has no Java menus: the label rule never applies there, so nothing needs recording.
        assert_eq!(plan.notes, Vec::<Noted>::new());
    }

    #[test]
    fn without_thediscdb_expendables_plans_its_two_labelled_cuts_as_editions() {
        let log = pinned_log(EXPENDABLES, EXPENDABLES_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&log.disc, log.name_only(), &settings, &library));
        // 8.3 rule 2: two cuts with recognised suffixes, 0.90, each an edition (8.4: the theatrical cut is named beside
        // another cut).
        assert_eq!(
            plan.titles,
            vec![
                planned("t00-00756", 41, feature(Some("Unrated Cut")), 7891, 28_831_358_976, Some("00756.mpls")),
                planned("t01-00820", 54, feature(Some("Theatrical Cut")), 7587, 27_736_528_896, Some("00820.mpls")),
            ]
        );
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Labels));
        assert_eq!((plan.title_confidence, plan.mode), (0.90, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown]);
        assert_eq!(plan.notes, Vec::<Noted>::new());
    }

    #[test]
    fn the_label_variant_is_rechosen_once_the_original_language_is_known() {
        let disc = orphanage();
        let name = disc_name(None, disc.volume_label.as_deref());
        let search = orphanage_search();
        let (settings, library) = (Settings::default(), LibraryFacts::default());

        // English original: the first pass's variant (title 1, without a language suffix) stays.
        let english = orphanage_record("en", Some(100));
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, Some(&english)), &settings, &library));
        assert_eq!((plan.identity.step, plan.identity.confidence), (IdentityStep::TmdbOne, 0.90));
        assert_eq!(plan.identity.tmdb, Some(movie_id(6537)));
        assert_eq!(plan.titles, vec![planned("t00-00001", 1, feature(None), 6000, 30_000_000_000, Some("00001.mpls"))]);
        // No second pass: an English original wants English only (`original_two_letter` keeps TMDb's "en").
        assert_eq!(plan.wanted, wanted_languages(Some("en")));
        assert!(plan.wanted.original_is_english && plan.wanted.original.is_empty());

        // Spanish original: the second pass picks the variant whose audio covers English and Spanish (title 2); the cut,
        // its edition and the confidence do not change.
        let spanish = orphanage_record("es", Some(100));
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, Some(&spanish)), &settings, &library));
        assert_eq!(plan.identity.original_language.as_deref(), Some("es"));
        assert_eq!(plan.titles, vec![planned("t00-00002", 2, feature(None), 6000, 31_000_000_000, Some("00002.mpls"))]);
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Labels));
        assert_eq!((plan.title_confidence, plan.mode), (0.90, Mode::Direct));
        assert_eq!(plan.wanted, wanted_languages(Some("es")));
        assert_eq!(plan.wanted.original, vec!["spa".to_string()]);
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(plan.notes, Vec::<Noted>::new());
    }

    #[test]
    fn with_tmdb_only_autofile_off_a_search_identity_is_capped_at_0_84_and_held() {
        let disc = orphanage();
        let name = disc_name(None, disc.volume_label.as_deref());
        let search = orphanage_search();
        let record = orphanage_record("en", Some(100));
        let library = LibraryFacts::default();

        let on = Settings::default();
        assert!(on.tmdb_only_autofile);
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &on, &library));
        assert_eq!((plan.identity_confidence, plan.identity.review), (0.90, None));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());

        let off = Settings { tmdb_only_autofile: false, ..Settings::default() };
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &off, &library));
        assert_eq!(plan.identity.step, IdentityStep::TmdbOne);
        assert_eq!((plan.identity_confidence, plan.identity.review), (0.84, Some(ReviewReason::IdentityAmbiguous)));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityAmbiguous]);
        // The mode follows the title confidence only (6.2, 6.3); the review reason holds the job later (8.8).
        assert_eq!((plan.title_confidence, plan.mode), (0.90, Mode::Direct));
    }

    #[test]
    fn several_plausible_results_hold_the_identity_at_0_50() {
        let disc = orphanage();
        let name = disc_name(None, disc.volume_label.as_deref());
        let text = search_json(&[
            ("movie", 6537, "The Orphanage", "El orfanato", "2007-05-20"),
            ("movie", 9999, "The Orphanage", "The Orphanage", "1990-01-01"),
        ]);
        let search = parse_search(&text, "search.json").unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, None), &settings, &library));
        assert_eq!(plan.identity.step, IdentityStep::TmdbSeveral);
        assert_eq!((plan.identity_confidence, plan.identity.review), (0.50, Some(ReviewReason::IdentityAmbiguous)));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityAmbiguous]);
    }

    #[test]
    fn a_search_identity_of_the_other_kind_than_the_planned_disc_is_held() {
        // One 100-minute title named SHOW, without Java: one group and no season marker, so 8.2 says movie and rule 3
        // plans it at 0.85. TMDb's one plausible result is a show whose episodes last 100 minutes, so its own runtime
        // check (TV: 100 against the 100-minute group) passes at step 4 although the disc is planned as a movie.
        let disc = made_disc(false, "SHOW", &[made(1, "1:40:00", "16", "00001.mpls", "1", "30000000000", "", ENG)]);
        let name = disc_name(None, disc.volume_label.as_deref());
        let search = show_search();
        let episodes: &[(u32, &str, Option<u32>)] = &[(1, "One", Some(100)), (2, "Two", Some(100))];
        let record = parse_tv(&tmdb_tv_json(99, "Show", "2010-01-01", "en", &[(1, episodes)]), "tv.json").unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &settings, &library));
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::TmdbOne, 0.90));
        assert_eq!((plan.identity.tmdb, plan.identity.review), (Some(show_id(99)), None));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
        assert_eq!(plan.titles, vec![planned("t00-00001", 1, feature(None), 6000, 30_000_000_000, Some("00001.mpls"))]);
        // Not settled by the spec (left to the owner): the identity's kind contradicts the plan, so D1 holds the disc,
        // and the planned titles are held at 0.60 so the disc is copied whole (6.3, 8.8 "Limits of D1").
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityAmbiguous, ReviewReason::TitleAmbiguous]);
        assert!(plan
            .evidence
            .sentences
            .contains(&"the TMDb identity is TV but the disc is planned as a movie".to_string()));
        assert!(plan.evidence.sentences.contains(
            &"the kind contradiction holds the titles at 0.60 with title_ambiguous, for whole-disc copy mode (6.3, 8.8 \
              Limits of D1)"
                .to_string()
        ));
        assert_eq!(plan.session, None);
    }

    #[test]
    fn firefly_disc_1_without_thediscdb_and_a_tv_search_identity_is_copied_whole() {
        // Firefly disc 1 (DVD) with TheDiscDB unreachable: the double-length pilot defeats 8.2's 15% spread, there is no
        // season marker and a DVD has no play-all rule, so the disc is a movie and rule 3 keeps the pilot alone at 0.85.
        // TMDb's one plausible result is the show, whose episodes last about 43 minutes, so its own TV check passes at
        // step 4. Copy mode keeps every title on staging, so review can assign the episodes without the disc (D1).
        let log = pinned_log(FIREFLY_1, FIREFLY_1_API);
        let search = parse_search(&search_json(&[("tv", 1437, "Firefly", "Firefly", "2002-09-20")]), "search.json").unwrap();
        let episodes: &[(u32, &str, Option<u32>)] =
            &[(1, "One", Some(86)), (2, "Two", Some(43)), (3, "Three", Some(43)), (4, "Four", Some(43))];
        let record = parse_tv(&tmdb_tv_json(1437, "Firefly", "2002-09-20", "en", &[(1, episodes)]), "tv.json").unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&log.disc, searched(log.name.as_ref(), &search, Some(&record)), &settings, &library));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
        assert_eq!(plan.identity.step, IdentityStep::TmdbOne);
        assert_eq!(plan.identity.tmdb, Some(show_id(1437)));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityAmbiguous, ReviewReason::TitleAmbiguous]);
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
    }

    #[test]
    fn a_search_identity_of_the_planned_kind_keeps_the_structural_plan_in_direct_mode() {
        // The same disc and search with a movie result: the kinds agree, so rule 3's 0.85 and Direct mode stand.
        let disc = made_disc(false, "SHOW", &[made(1, "1:40:00", "16", "00001.mpls", "1", "30000000000", "", ENG)]);
        let name = disc_name(None, disc.volume_label.as_deref());
        let search = parse_search(&search_json(&[("movie", 99, "Show", "Show", "2010-01-01")]), "search.json").unwrap();
        let record = parse_movie(&tmdb_movie_json(99, "Show", "Show", "2010-01-01", "en", Some(100)), "movie.json").unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &settings, &library));
        assert_eq!((plan.identity.step, plan.identity.tmdb), (IdentityStep::TmdbOne, Some(movie_id(99))));
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
    }

    #[test]
    fn an_unnamed_cut_holds_the_plan_at_0_60_in_copy_mode() {
        let disc = made_disc(
            true,
            "TWO_CUTS",
            &[
                made(1, "1:40:00", "16", "00001.mpls", "1", "30000000000", "FPL_MainFeature", ENG),
                made(2, "1:40:00", "16", "00002.mpls", "2", "31000000000", "FPL_MainFeature_DC", ENG),
            ],
        );
        let name = disc_name(None, disc.volume_label.as_deref());
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { name: name.as_ref(), ..LookupInputs::default() };
        let plan = run(&inputs(&disc, lookups, &settings, &library));
        assert_eq!(
            plan.titles,
            vec![
                planned("t00-00001", 1, feature(Some("Theatrical Cut")), 6000, 30_000_000_000, Some("00001.mpls")),
                planned("t01-00002", 2, feature(None), 6000, 31_000_000_000, Some("00002.mpls")),
            ]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown, ReviewReason::UnnamedCut]);
    }

    #[test]
    fn feature_min_minutes_decides_which_groups_are_rivals() {
        let disc = two_rivals();
        let library = LibraryFacts::default();

        // The default 45: both titles are kept and are rivals (47 is within 10% of 50) with different content (rule
        // 3.7). Structural rivals carry no edition name.
        let default = Settings::default();
        assert_eq!(default.feature_min_minutes, 45);
        let plan = run(&inputs(&disc, LookupInputs::default(), &default, &library));
        assert_eq!(
            plan.titles,
            vec![
                planned("t00-00001", 1, feature(None), 3000, 4_000_000_000, Some("00001.mpls")),
                planned("t01-00002", 2, feature(None), 2820, 3_000_000_000, Some("00002.mpls")),
            ]
        );
        assert_eq!(plan.evidence.rivals, vec!["1".to_string(), "2".to_string()]);
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown, ReviewReason::MultipleFeatures]);

        // 48: only the 50-minute title is kept, one rival (rule 3.5).
        let longer = Settings { feature_min_minutes: 48, ..Settings::default() };
        let plan = run(&inputs(&disc, LookupInputs::default(), &longer, &library));
        assert_eq!(plan.titles, vec![planned("t00-00001", 1, feature(None), 3000, 4_000_000_000, Some("00001.mpls"))]);
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown]);
    }

    #[test]
    fn minlength_s_decides_the_candidates() {
        let disc = made_disc(
            false,
            "SHORT_AND_LONG",
            &[
                made(1, "0:01:40", "0", "00001.mpls", "1", "100000000", "", ENG),
                made(2, "1:40:00", "16", "00002.mpls", "2", "30000000000", "", ENG),
            ],
        );
        let library = LibraryFacts::default();
        let default = Settings::default();
        assert_eq!(default.minlength_s, 120);
        let plan = run(&inputs(&disc, LookupInputs::default(), &default, &library));
        assert_eq!(plan.evidence.candidates, vec![2]);
        assert_eq!(plan.evidence.groups.len(), 1);

        let shorter = Settings { minlength_s: 60, ..Settings::default() };
        let plan = run(&inputs(&disc, LookupInputs::default(), &shorter, &library));
        assert_eq!(plan.evidence.candidates, vec![1, 2]);
        assert_eq!(plan.evidence.groups.len(), 2);
        // The 100-second title is a candidate but not a feature: the plan is the same.
        assert_eq!(plan.titles, vec![planned("t00-00002", 2, feature(None), 6000, 30_000_000_000, Some("00002.mpls"))]);
    }

    #[test]
    fn auto_threshold_decides_the_mode() {
        let log = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API);
        let library = LibraryFacts::default();
        let default = Settings::default();
        assert_eq!(default.auto_threshold, 0.85);
        let plan = run(&inputs(&log.disc, log.name_only(), &default, &library));
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));

        let stricter = Settings { auto_threshold: 0.90, ..Settings::default() };
        let plan = run(&inputs(&log.disc, log.name_only(), &stricter, &library));
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Copy));
    }

    #[test]
    fn an_unmapped_tv_disc_is_suggested_by_runtime_at_0_60_with_tv_unmapped() {
        let disc = episodes_disc("SHOW_S2_D1", true);
        let name = disc_name(None, disc.volume_label.as_deref());
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { name: name.as_ref(), ..LookupInputs::default() };
        let plan = run(&inputs(&disc, lookups, &settings, &library));
        // The play-all title makes the groups' lengths differ by more than 15%, so the season marker decides (8.2).
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::SeasonMarker));
        assert_eq!(plan.evidence.groups.len(), 4);
        // Title 4 (play all) is never suggested; the season comes from the name, the episodes start at 1 (8.5).
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, Some(2), &[1], None),
                suggested("t01-00002", 2, Some(2), &[2], None),
                suggested("t02-00003", 3, Some(2), &[3], None),
            ]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown, ReviewReason::TvUnmapped]);
        assert_eq!(plan.session, None);
        assert_eq!(plan.evidence.title_rule, None);
        // TV by its season marker: 8.3 rule 2 is never reached, so no label-rule note (10.2).
        assert_eq!(plan.notes, Vec::<Noted>::new());
    }

    #[test]
    fn a_tv_search_identity_names_the_suggested_episodes_from_its_season() {
        let disc = episodes_disc("SHOW_S2_D1", true);
        let name = disc_name(None, disc.volume_label.as_deref());
        let search = show_search();
        let record = show_record(2);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &settings, &library));
        // 8.1 step 4 for TV: season 2's median episode (22 minutes) is within 15% of the median episode-length group
        // (22.5 minutes, the play-all group left out).
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::TmdbOne, 0.90));
        assert_eq!(plan.identity.tmdb, Some(show_id(99)));
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, Some(2), &[1], Some("One")),
                suggested("t01-00002", 2, Some(2), &[2], Some("Two")),
                suggested("t02-00003", 3, Some(2), &[3], Some("Three")),
            ]
        );
        assert_eq!(plan.reviews, vec![ReviewReason::TvUnmapped]);
        // An unmapped disc does not join a session until review confirms its episodes.
        assert_eq!(plan.session, None);
    }

    #[test]
    fn an_unmapped_disc_continues_its_season_session_from_the_next_unassigned_episode() {
        let disc = episodes_disc("SHOW_S2_D1", true);
        let name = disc_name(None, disc.volume_label.as_deref());
        let search = show_search();
        let record = show_record(2);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let sessions = [session_with(99, 2, None, "EARLIER", &[(1, 7), (2, 8), (3, 9)])];
        let mut planned_inputs = inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &settings, &library);
        planned_inputs.sessions = &sessions;
        let plan = run(&planned_inputs);
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, Some(2), &[4], Some("Four")),
                suggested("t01-00002", 2, Some(2), &[5], Some("Five")),
                suggested("t02-00003", 3, Some(2), &[6], Some("Six")),
            ]
        );
        assert_eq!(plan.session, None);
    }

    #[test]
    fn without_a_parsed_season_the_shows_single_open_session_gives_the_season() {
        let disc = episodes_disc("SHOW_D2", false);
        let name = disc_name(None, disc.volume_label.as_deref());
        assert_eq!(name.as_ref().map(|name| name.season), Some(None));
        let search = show_search();
        let record = show_record(3);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        // Season 1 is closed (both of its episodes assigned); season 3 is the show's one open session.
        let sessions = [
            session_with(99, 1, Some(2), "OLDER", &[(1, 0), (2, 1)]),
            session_with(99, 3, None, "EARLIER", &[(1, 7), (2, 8)]),
        ];
        assert!(sessions[0].closed);
        let mut planned_inputs = inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &settings, &library);
        planned_inputs.sessions = &sessions;
        let plan = run(&planned_inputs);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Structure));
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, Some(3), &[3], Some("Three")),
                suggested("t01-00002", 2, Some(3), &[4], Some("Four")),
                suggested("t02-00003", 3, Some(3), &[5], Some("Five")),
            ]
        );
    }

    #[test]
    fn without_a_season_or_a_session_the_suggestions_have_no_season_and_keep_title_order() {
        let disc = episodes_disc("SHOW_D2", false);
        let name = disc_name(None, disc.volume_label.as_deref());
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { name: name.as_ref(), ..LookupInputs::default() };
        let plan = run(&inputs(&disc, lookups, &settings, &library));
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, None, &[1], None),
                suggested("t01-00002", 2, None, &[2], None),
                suggested("t02-00003", 3, None, &[3], None),
            ]
        );
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown, ReviewReason::TvUnmapped]);
        assert_eq!(plan.session, None);
    }

    #[test]
    fn with_several_open_sessions_and_no_parsed_season_review_asks_for_the_season() {
        let disc = episodes_disc("SHOW_D2", false);
        let name = disc_name(None, disc.volume_label.as_deref());
        let search = show_search();
        let record = show_record(3);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        // Seasons 2 and 3 are both open, so neither is the show's single open session (8.5).
        let sessions = [session_with(99, 2, None, "A", &[(1, 7)]), session_with(99, 3, None, "B", &[(1, 8)])];
        let mut planned_inputs = inputs(&disc, searched(name.as_ref(), &search, Some(&record)), &settings, &library);
        planned_inputs.sessions = &sessions;
        let plan = run(&planned_inputs);
        assert_eq!(plan.identity.tmdb, Some(show_id(99)));
        // 8.5 "failing that, suggestions start at the first episode and review asks for the season".
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, None, &[1], None),
                suggested("t01-00002", 2, None, &[2], None),
                suggested("t02-00003", 3, None, &[3], None),
            ]
        );
        assert_eq!(plan.reviews, vec![ReviewReason::TvUnmapped]);
        assert_eq!(plan.session, None);
        assert!(plan
            .evidence
            .sentences
            .contains(&"the show has several open season sessions: review asks for the season (8.5)".to_string()));
    }

    #[test]
    fn a_play_all_title_makes_a_movie_disc_tv_and_is_never_suggested() {
        // Three titles of 22 to 23 minutes and their play-all (title 4): the lengths differ by more than 15% and the
        // name has no season marker, so 8.2 says movie; 8.3 rule 3.1 then finds the play-all title: the disc is TV (8.5).
        let disc = episodes_disc("BOXSET", true);
        let name = disc_name(None, disc.volume_label.as_deref());
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { name: name.as_ref(), ..LookupInputs::default() };
        let plan = run(&inputs(&disc, lookups, &settings, &library));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::PlayAll));
        // The play-all group stays in the evidence but is never suggested and never counts as an episode.
        assert_eq!(plan.evidence.groups.len(), 4);
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, None, &[1], None),
                suggested("t01-00002", 2, None, &[2], None),
                suggested("t02-00003", 3, None, &[3], None),
            ]
        );
        assert_eq!(plan.evidence.title_rule, None);
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown, ReviewReason::TvUnmapped]);
        // Rule 2 was reached (rule 1 had no record) and MakeMKV did not report Java: the label rule's note (10.2).
        assert_eq!(
            plan.notes,
            vec![Noted { note: Note::LabelRuleUnavailable, stream: None, detail: LABEL_RULE_UNAVAILABLE.to_string() }]
        );
        assert!(plan
            .evidence
            .sentences
            .contains(&"8.3 rule 3.1: group 1,2,3 is a play-all title, so the disc is TV (8.5)".to_string()));
    }

    #[test]
    fn every_play_all_title_is_dropped_from_the_suggestions() {
        // Five episodes (segments 1 to 5) and two play-all titles: title 6 plays 1,2,3 and title 7 plays 4,5. The season
        // marker makes the disc TV (8.2); 8.5 "play all titles are dropped".
        let disc = made_disc(
            false,
            "SHOW_S1_D1",
            &[
                made(1, "0:22:00", "5", "00001.mpls", "1", "4000000000", "", ENG),
                made(2, "0:22:30", "5", "00002.mpls", "2", "4100000000", "", ENG),
                made(3, "0:23:00", "5", "00003.mpls", "3", "4200000000", "", ENG),
                made(4, "0:22:00", "5", "00004.mpls", "4", "4000000000", "", ENG),
                made(5, "0:22:30", "5", "00005.mpls", "5", "4100000000", "", ENG),
                made(6, "1:07:30", "15", "00006.mpls", "1,2,3", "12300000000", "", ENG),
                made(7, "0:44:30", "10", "00007.mpls", "4,5", "8100000000", "", ENG),
            ],
        );
        let name = disc_name(None, disc.volume_label.as_deref());
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { name: name.as_ref(), ..LookupInputs::default() };
        let plan = run(&inputs(&disc, lookups, &settings, &library));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::SeasonMarker));
        assert_eq!(plan.evidence.groups.len(), 7);
        assert_eq!(
            plan.titles.iter().map(|planned| (planned.title, planned.role.clone(), planned.suggested)).collect::<Vec<_>>(),
            (1..=5).map(|number| (number, episode(Some(1), &[number], None), true)).collect::<Vec<_>>()
        );
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown, ReviewReason::TvUnmapped]);
    }

    #[test]
    fn a_play_all_title_on_a_disc_thediscdb_calls_a_movie_leaves_it_a_movie() {
        // An extras play-all (title 4, segments 1,2,3) beside a 100-minute feature (title 5), without Java. TheDiscDB
        // knows the disc as a movie but lists no title, so rule 1 gives no plan and rule 3 decides (8.3).
        let disc = made_disc(
            false,
            "FILM_DISC",
            &[
                made(1, "0:22:00", "5", "00001.mpls", "1", "4000000000", "", ENG),
                made(2, "0:22:30", "5", "00002.mpls", "2", "4100000000", "", ENG),
                made(3, "0:23:00", "5", "00003.mpls", "3", "4200000000", "", ENG),
                made(4, "1:07:30", "15", "00004.mpls", "1,2,3", "12300000000", "", ENG),
                made(5, "1:40:00", "16", "00005.mpls", "5", "30000000000", "", ENG),
            ],
        );
        let name = disc_name(None, disc.volume_label.as_deref());
        // A TheDiscDB answer of the API's shape: `media.type` "Movie" with a TMDb ID, and no title.
        let record: DiscDbRecord = serde_json::from_value(json!({
            "media": {"title": "Film", "year": 2010, "type": "Movie", "external_ids": {"tmdb": "77"}},
            "titles": []
        }))
        .unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { name: name.as_ref(), discdb_by_fingerprint: Some(&record), ..LookupInputs::default() };
        let plan = run(&inputs(&disc, lookups, &settings, &library));
        assert_discdb_one(&plan, movie_id(77), "Film", 2010);
        // 8.2: TheDiscDB states the kind, so 8.3 rule 3.1's play-all does not make the disc TV.
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        assert_eq!(plan.titles, vec![planned("t00-00005", 5, feature(None), 6000, 30_000_000_000, Some("00005.mpls"))]);
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
        assert_eq!(plan.evidence.rivals, vec!["5".to_string()]);
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert!(!plan.reviews.contains(&ReviewReason::TvUnmapped));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert!(plan.evidence.sentences.contains(
            &"8.3 rule 3.1: group 1,2,3 is play-all shaped, but TheDiscDB states the disc is a movie (8.2), so rule 3 runs \
              without 3.1"
                .to_string()
        ));
        // Rule 1 gave no plan and MakeMKV did not report Java: the label rule's note (10.2).
        assert_eq!(
            plan.notes,
            vec![Noted { note: Note::LabelRuleUnavailable, stream: None, detail: LABEL_RULE_UNAVAILABLE.to_string() }]
        );
    }

    #[test]
    fn a_play_all_shaped_feature_on_a_disc_thediscdb_calls_a_movie_is_an_ordinary_rival() {
        // The feature (title 3, 00800.mpls) plays a 100-minute body clip and a 10-minute credits clip, which MakeMKV also
        // lists as titles 1 and 2 (00001.m2ts, 00002.m2ts). TheDiscDB knows the disc as a movie but lists no title, so
        // rule 1 gives no plan and rule 3 decides without 3.1: the 110-minute "1,2" and the 100-minute "1" are rivals
        // (within 10%), whose segments differ by half, so the plan is 0.60 multiple_features in copy mode. Dropping the
        // play-all-shaped group would have planned the body clip alone at 0.85 and filed the movie without its last clip.
        let disc = made_disc(
            false,
            "FILM_DISC",
            &[
                made(1, "1:40:00", "15", "00001.m2ts", "1", "28000000000", "", ENG),
                made(2, "0:10:00", "1", "00002.m2ts", "2", "2800000000", "", ENG),
                made(3, "1:50:00", "16", "00800.mpls", "1,2", "30800000000", "", ENG),
            ],
        );
        let name = disc_name(None, disc.volume_label.as_deref());
        let record: DiscDbRecord = serde_json::from_value(json!({
            "media": {"title": "Film", "year": 2010, "type": "Movie", "external_ids": {"tmdb": "77"}},
            "titles": []
        }))
        .unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { name: name.as_ref(), discdb_by_fingerprint: Some(&record), ..LookupInputs::default() };
        let plan = run(&inputs(&disc, lookups, &settings, &library));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
        assert_eq!(plan.evidence.rivals, vec!["1".to_string(), "1,2".to_string()]);
        assert_eq!(plan.titles.iter().map(|planned| planned.title).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::MultipleFeatures]);
    }

    #[test]
    fn several_search_results_all_of_the_other_kind_hold_the_disc_planned_as_a_movie() {
        // The disc of a_search_identity_of_the_other_kind_than_the_planned_disc_is_held (one 100-minute title named
        // SHOW, planned as a movie at 0.85), with two plausible shows of that name: identify picks none (none is of the
        // planned kind), and every plausible result is TV, so the kind contradiction holds the titles for copy mode.
        let disc = made_disc(false, "SHOW", &[made(1, "1:40:00", "16", "00001.mpls", "1", "30000000000", "", ENG)]);
        let name = disc_name(None, disc.volume_label.as_deref());
        let two_shows = search_json(&[("tv", 99, "Show", "Show", "2010-01-01"), ("tv", 98, "Show", "Show", "2005-03-24")]);
        let search = parse_search(&two_shows, "search.json").unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, None), &settings, &library));
        assert_eq!((plan.identity.step, plan.identity.tmdb), (IdentityStep::TmdbSeveral, None));
        assert_eq!(plan.identity.plausible, vec![show_id(99), show_id(98)]);
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityAmbiguous, ReviewReason::TitleAmbiguous]);
        assert!(plan
            .evidence
            .sentences
            .contains(&"every plausible TMDb result is TV but the disc is planned as a movie".to_string()));
        // A mixed-kind list does not hold the titles: identify picks the movie, the kinds agree, and rule 4 remains the
        // guard.
        let mixed = search_json(&[("tv", 99, "Show", "Show", "2010-01-01"), ("movie", 97, "Show", "Show", "2012-06-01")]);
        let search = parse_search(&mixed, "search.json").unwrap();
        let plan = run(&inputs(&disc, searched(name.as_ref(), &search, None), &settings, &library));
        assert_eq!((plan.identity.step, plan.identity.tmdb), (IdentityStep::TmdbSeveral, Some(movie_id(97))));
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityAmbiguous]);
    }

    #[test]
    fn a_mapped_disc_without_a_fingerprint_starts_an_empty_session() {
        let log = pinned_log(CHERNOBYL_1, CHERNOBYL_1_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&log.disc, log.lookups(), &settings, &library));
        assert_eq!(plan.session, Some(SeasonSession::new(87108, 1, None)));
    }

    #[test]
    fn a_mapped_disc_joins_its_existing_season_session() {
        let log = pinned_log(FIREFLY_2, FIREFLY_2_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let sessions = [session_with(1437, 1, Some(14), FIREFLY_1_API, &[(1, 0), (2, 1), (3, 2)])];
        let mut planned_inputs = inputs(&log.disc, log.lookups(), &settings, &library);
        planned_inputs.fingerprint = Some(log.fingerprint.as_str());
        planned_inputs.sessions = &sessions;
        let plan = run(&planned_inputs);
        let first = FIREFLY_1_API.to_string();
        let second = FIREFLY_2_API.to_string();
        assert_eq!(
            plan.session,
            Some(SeasonSession {
                show_id: 1437,
                season: 1,
                discs: vec![first.clone(), second.clone()],
                assigned: vec![
                    assigned_episode(1, &first, 0),
                    assigned_episode(2, &first, 1),
                    assigned_episode(3, &first, 2),
                    assigned_episode(4, &second, 0),
                    assigned_episode(5, &second, 1),
                    assigned_episode(6, &second, 2),
                    assigned_episode(7, &second, 5),
                ],
                episode_count: Some(14),
                closed: false,
            })
        );
    }

    #[test]
    fn a_mapped_disc_whose_identity_is_not_confirmed_joins_no_session() {
        let log = pinned_log(CHERNOBYL_1, CHERNOBYL_1_API);
        // TheDiscDB's answer without its TMDb ID: TMDb is searched with its title and year (8.1), and the one plausible
        // result has another year (2018 against 2019), so the identity is step 5 (Task 4), not confirmed.
        let mut record = log.record.clone();
        record.media.as_mut().expect("the answer has media").external_ids.tmdb = None;
        let search =
            parse_search(&search_json(&[("tv", 87108, "Chernobyl", "Chernobyl", "2018-05-06")]), "search.json").unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { discdb_by_fingerprint: Some(&record), search: Some(&search), ..log.name_only() };
        let mut planned_inputs = inputs(&log.disc, lookups, &settings, &library);
        planned_inputs.fingerprint = Some(log.fingerprint.as_str());
        let plan = run(&planned_inputs);
        assert_eq!(plan.identity.step, IdentityStep::TmdbSeveral);
        assert_eq!((plan.identity.tmdb, plan.identity_confidence), (Some(show_id(87108)), 0.50));
        assert!(plan.identity.evidence.contains(&"tv 87108 is not TheDiscDB's kind and year".to_string()));
        // TheDiscDB still maps the episodes (8.5: 0.98), but "A disc joins a session when its identity is confirmed".
        assert_eq!((plan.kind, plan.kind_source), (Kind::Tv, KindSource::Discdb));
        assert_eq!(plan.titles.iter().map(|planned| planned.title).collect::<Vec<u32>>(), vec![1, 8, 7]);
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityAmbiguous]);
        assert_eq!(plan.session, None);
    }

    #[test]
    fn a_new_session_takes_its_episode_count_from_the_record_and_closes_when_complete() {
        let log = pinned_log(FIREFLY_1, FIREFLY_1_API);
        let episodes: &[(u32, &str, Option<u32>)] =
            &[(1, "Serenity", Some(86)), (2, "The Train Job", Some(43)), (3, "Bushwhacked", Some(43))];
        let text = tmdb_tv_json(1437, "Firefly", "2002-09-20", "en", &[(1, episodes)]);
        let record = parse_tv(&text, "tv.json").unwrap();
        let count = record.season(1).and_then(|season| season.episode_count);
        assert_eq!(count, Some(3));
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let mut lookups = log.lookups();
        lookups.record = Some(&record);
        let mut planned_inputs = inputs(&log.disc, lookups, &settings, &library);
        planned_inputs.fingerprint = Some(log.fingerprint.as_str());
        let plan = run(&planned_inputs);
        let session = plan.session.expect("the disc joins season 1");
        assert_eq!(session.episode_count, count);
        assert_eq!(session.assigned.len(), 3);
        assert!(session.closed, "every episode of the season is assigned");
    }

    #[test]
    fn a_pending_lookup_is_recorded_and_planning_proceeds_on_structure() {
        let log = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { discdb_pending: true, tmdb_pending: true, ..log.name_only() };
        let plan = run(&inputs(&log.disc, lookups, &settings, &library));
        assert!(plan.identity.lookup_pending);
        assert_eq!(plan.identity.step, IdentityStep::Nothing);
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
        assert_eq!(plan.titles, vec![planned("t00-00001", 0, feature(None), 6073, 67_757_930_496, Some("00001.mpls"))]);
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        // The sentence says the lookup is pending, not that TheDiscDB gave no record.
        assert!(plan
            .evidence
            .sentences
            .contains(&"the TheDiscDB lookup is pending: the titles are planned from the scan".to_string()));
        assert!(!plan.evidence.sentences.iter().any(|sentence| sentence.starts_with("TheDiscDB gave no record")));
        let unanswered = run(&inputs(&log.disc, log.name_only(), &settings, &library));
        assert!(unanswered
            .evidence
            .sentences
            .contains(&"TheDiscDB gave no record: the titles are planned from the scan".to_string()));
    }

    #[test]
    fn a_thediscdb_record_by_disc_id_alone_plans_like_one_by_fingerprint() {
        let log = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { discdb_by_disc_id: Some(&log.record), ..log.name_only() };
        let plan = run(&inputs(&log.disc, lookups, &settings, &library));
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::DiscdbOne, 0.95));
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
        assert_eq!(plan.titles, vec![planned("t00-00001", 0, feature(None), 6073, 67_757_930_496, Some("00001.mpls"))]);
        assert_eq!(plan.title_confidence, 0.98);
    }

    #[test]
    fn thediscdb_by_disc_id_and_fingerprint_gives_0_99() {
        let log = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API);
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { discdb_by_disc_id: Some(&log.record), ..log.lookups() };
        let mut planned_inputs = inputs(&log.disc, lookups, &settings, &library);
        planned_inputs.disc_id = Some("0123456789ABCDEF0123456789ABCDEF01234567");
        let plan = run(&planned_inputs);
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::DiscdbBoth, 0.99));
        assert_eq!(plan.title_confidence, 0.98);
        assert!(plan.evidence.sentences.contains(&"disc ID 0123456789ABCDEF0123456789ABCDEF01234567".to_string()));
    }

    #[test]
    fn a_lone_theatrical_cut_is_named_when_the_library_folder_holds_an_edition_named_file() {
        let log = pinned_log(SMILE, SMILE_API);
        let settings = Settings::default();
        let library = LibraryFacts { folder_has_edition_named_file: true, ..LibraryFacts::default() };
        let plan = run(&inputs(&log.disc, log.lookups(), &settings, &library));
        assert_eq!(plan.titles[0].role, feature(Some("Theatrical Cut")));
    }

    #[test]
    fn an_earlier_filed_copy_names_the_cut_the_same_way() {
        let log = pinned_log(EXPENDABLES, EXPENDABLES_API);
        let settings = Settings::default();
        let library = LibraryFacts {
            earlier_edition_names: BTreeMap::from([("unrated cut".to_string(), "Unrated".to_string())]),
            ..LibraryFacts::default()
        };
        let plan = run(&inputs(&log.disc, log.lookups(), &settings, &library));
        assert_eq!(plan.titles[0].role, feature(Some("Unrated")));
    }

    #[test]
    fn a_database_identity_takes_its_original_language_from_the_tmdb_record() {
        let log = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API);
        // A synthetic record for TheDiscDB's TMDb ID with a Spanish original language (TMDb's documented shape).
        let text = tmdb_movie_json(245891, "John Wick", "John Wick", "2014-10-22", "es", Some(101));
        let record = parse_movie(&text, "movie.json").unwrap();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let lookups = LookupInputs { record: Some(&record), ..log.lookups() };
        let plan = run(&inputs(&log.disc, lookups, &settings, &library));
        assert_eq!(plan.identity.step, IdentityStep::DiscdbOne);
        assert_eq!(plan.wanted, wanted_languages(Some("es")));
        assert_eq!(plan.titles[0].title, 0);
    }

    #[test]
    fn a_disc_without_titles_cannot_be_planned() {
        let disc = disc_of(&scan_text(BLURAY, &[]));
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        assert_eq!(
            plan(&inputs(&disc, LookupInputs::default(), &settings, &library)),
            Err("the scan lists no title, so there is nothing to plan".to_string())
        );
    }

    #[test]
    fn the_plan_document_round_trips_through_json_in_field_order() {
        let plan = pinned_log(CHERNOBYL_1, CHERNOBYL_1_API).plan();
        let value = serde_json::to_value(&plan).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "identity",
                "kind",
                "kind_source",
                "titles",
                "mode",
                "identity_confidence",
                "title_confidence",
                "reviews",
                "notes",
                "wanted",
                "session",
                "evidence"
            ]
        );
        assert_eq!(value["kind"], json!("tv"));
        assert_eq!(value["mode"], json!("direct"));
        assert_eq!(value["titles"][0]["key"], json!("t00-00801"));
        assert_eq!(value["titles"][0]["role"]["role"], json!("episode"));
        assert_eq!(
            serde_json::to_value(&plan.titles[0].role).unwrap(),
            json!({"role": "episode", "season": 1, "episodes": [1], "name": "1.23.45"})
        );
        assert_eq!(serde_json::to_value(feature(None)).unwrap(), json!({"role": "feature", "edition": null}));
        assert_eq!(serde_json::from_value::<Plan>(value).unwrap(), plan);
    }

    use crate::remembered::{RememberedTitle, TitleOverrideEntry, TitleRole};

    /// A remembered title of a Blu-ray (attribute 16, segment map, seconds, role).
    fn remembered_title(playlist: &str, segments: &str, seconds: u32, role: TitleRole) -> RememberedTitle {
        RememberedTitle {
            source_file: Some(playlist.to_string()),
            dvd_title: None,
            segment_map: segments.to_string(),
            seconds,
            role,
        }
    }

    /// A remembered decision for the film TMDb 6537 with these titles and no track overrides.
    fn remembered_film(titles: Vec<RememberedTitle>) -> Remembered {
        Remembered { fingerprint: "FP".to_string(), tmdb: movie_id(6537), titles, audio: Vec::new(), subtitles: Vec::new() }
    }

    /// One feature without Java: 100 minutes, playlist 00001.mpls, segment map 1, this size ("" leaves it out).
    fn one_feature(size: &str) -> Disc {
        made_disc(false, "ONE_FEATURE", &[made(1, "1:40:00", "16", "00001.mpls", "1", size, "", ENG)])
    }

    #[test]
    fn a_remembered_decision_sets_identity_and_title_confidence_to_1_00() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let remembered = remembered_film(vec![remembered_title("00002.mpls", "2", 2820, TitleRole::Feature)]);
        let mut planned_inputs = inputs(&disc, LookupInputs::default(), &settings, &library);
        planned_inputs.remembered = Some(&remembered);
        let plan = run(&planned_inputs);
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Remembered, 1.00));
        assert_eq!(plan.identity.tmdb, Some(movie_id(6537)));
        assert_eq!(
            plan.titles,
            vec![PlannedTitle {
                remembered: true,
                ..planned("t00-00002", 2, feature(None), 2820, 3_000_000_000, Some("00002.mpls"))
            }]
        );
        assert_eq!((plan.title_confidence, plan.mode), (1.00, Mode::Direct));
        // The structure's multiple_features went with the planned titles; the remembered identity raises nothing.
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());

        // A remembered title that matches no scan title (its duration is 820 s off) leaves the planned titles.
        let elsewhere = remembered_film(vec![remembered_title("00002.mpls", "2", 2000, TitleRole::Feature)]);
        planned_inputs.remembered = Some(&elsewhere);
        let plan = run(&planned_inputs);
        assert_eq!(plan.identity_confidence, 1.00);
        assert_eq!(plan.titles.len(), 2);
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::MultipleFeatures]);
    }

    #[test]
    fn the_owners_assign_titles_win_over_a_remembered_decision() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let remembered = remembered_film(vec![remembered_title("00002.mpls", "2", 2820, TitleRole::Feature)]);
        let overrides = Overrides {
            titles: vec![TitleOverrideEntry { title: 1, role: TitleRole::Cut { edition: "Director's Cut".to_string() } }],
            ..Overrides::default()
        };
        let mut planned_inputs = inputs(&disc, LookupInputs::default(), &settings, &library);
        planned_inputs.remembered = Some(&remembered);
        planned_inputs.overrides = Some(&overrides);
        let plan = run(&planned_inputs);
        assert_eq!(
            plan.titles,
            vec![PlannedTitle {
                remembered: true,
                ..planned("t00-00001", 1, feature(Some("Director's Cut")), 3000, 4_000_000_000, Some("00001.mpls"))
            }]
        );
        assert_eq!(plan.title_confidence, 1.00);
        // Set identity was not used: the identity is the remembered one.
        assert_eq!(plan.identity.step, IdentityStep::Remembered);
    }

    #[test]
    fn remembered_cuts_are_named_as_editions() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let remembered = remembered_film(vec![
            remembered_title("00001.mpls", "1", 3000, TitleRole::Cut { edition: "Extended Cut".to_string() }),
            remembered_title("00002.mpls", "2", 2820, TitleRole::Feature),
        ]);
        let mut planned_inputs = inputs(&disc, LookupInputs::default(), &settings, &library);
        planned_inputs.remembered = Some(&remembered);
        let plan = run(&planned_inputs);
        // 8.4: the theatrical cut is named beside another cut.
        assert_eq!(
            plan.titles.iter().map(|planned| (planned.key.as_str(), planned.role.clone())).collect::<Vec<_>>(),
            vec![("t00-00001", feature(Some("Extended Cut"))), ("t01-00002", feature(Some("Theatrical Cut")))]
        );
        assert!(plan.titles.iter().all(|planned| planned.remembered && !planned.suggested));
    }

    #[test]
    fn a_remembered_decision_that_matches_only_some_of_its_titles_leaves_the_planned_titles() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        // The cut matches title 1 (00001.mpls, segment map 1, 3000 s); the feature's 2000 s are 820 s from title 2's.
        let remembered = remembered_film(vec![
            remembered_title("00001.mpls", "1", 3000, TitleRole::Cut { edition: "Extended Cut".to_string() }),
            remembered_title("00002.mpls", "2", 2000, TitleRole::Feature),
        ]);
        let mut planned_inputs = inputs(&disc, LookupInputs::default(), &settings, &library);
        planned_inputs.remembered = Some(&remembered);
        let plan = run(&planned_inputs);
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Remembered, 1.00));
        // Spec 5: the decision covers the scan only when every title it keeps matches; taken at 1.00 it would make a
        // Direct job that silently leaves the other cut out (8.8 "Limits of D1"). The titles are planned instead.
        assert_eq!(plan.titles.len(), 2);
        assert!(plan.titles.iter().all(|planned| !planned.remembered));
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::MultipleFeatures]);
        assert!(plan
            .evidence
            .sentences
            .contains(&"the remembered decision matches only 1 of its 2 titles: the titles are planned".to_string()));
    }

    #[test]
    fn ignored_and_unknown_assigned_titles_are_left_out() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let overrides = Overrides {
            titles: vec![
                TitleOverrideEntry { title: 1, role: TitleRole::Ignored },
                TitleOverrideEntry { title: 2, role: TitleRole::Feature },
                TitleOverrideEntry { title: 9, role: TitleRole::Feature },
            ],
            ..Overrides::default()
        };
        let mut planned_inputs = inputs(&disc, LookupInputs::default(), &settings, &library);
        planned_inputs.overrides = Some(&overrides);
        let plan = run(&planned_inputs);
        assert_eq!(
            plan.titles,
            vec![PlannedTitle {
                remembered: true,
                ..planned("t00-00002", 2, feature(None), 2820, 3_000_000_000, Some("00002.mpls"))
            }]
        );
        assert!(plan.evidence.sentences.contains(&"title 1 is ignored".to_string()));
        assert!(plan.evidence.sentences.contains(&"assigned title 9 is not in the scan and is left out".to_string()));
    }

    #[test]
    fn set_identity_sets_identity_confidence_to_1_00_and_leaves_the_title_reviews() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let overrides = Overrides { identity: Some(movie_id(6537)), ..Overrides::default() };
        let mut planned_inputs = inputs(&disc, LookupInputs::default(), &settings, &library);
        planned_inputs.overrides = Some(&overrides);
        let plan = run(&planned_inputs);
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Reviewed, 1.00));
        assert_eq!((plan.identity.tmdb, plan.identity.review), (Some(movie_id(6537)), None));
        // Set identity covers the identity only (8.8): the titles stay ambiguous.
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::MultipleFeatures]);
    }

    #[test]
    fn assigned_episodes_make_a_tv_plan_that_joins_the_season_session() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let overrides = Overrides {
            identity: Some(show_id(99)),
            titles: vec![
                TitleOverrideEntry { title: 1, role: TitleRole::Episodes { season: 2, episodes: vec![1] } },
                TitleOverrideEntry { title: 2, role: TitleRole::Episodes { season: 2, episodes: vec![2, 3] } },
            ],
            ..Overrides::default()
        };
        let mut planned_inputs = inputs(&disc, LookupInputs::default(), &settings, &library);
        planned_inputs.overrides = Some(&overrides);
        planned_inputs.fingerprint = Some("FP");
        let plan = run(&planned_inputs);
        assert_eq!(plan.kind, Kind::Tv);
        // two_rivals is a movie by the 8.2 default; the assigned episodes change the kind, and a sentence says so.
        assert!(plan.evidence.sentences.contains(&"the assigned titles make the disc TV".to_string()));
        assert_eq!(
            plan.titles,
            vec![
                PlannedTitle {
                    remembered: true,
                    ..planned("t00-00001", 1, episode(Some(2), &[1], None), 3000, 4_000_000_000, Some("00001.mpls"))
                },
                PlannedTitle {
                    remembered: true,
                    ..planned("t01-00002", 2, episode(Some(2), &[2, 3], None), 2820, 3_000_000_000, Some("00002.mpls"))
                },
            ]
        );
        assert_eq!((plan.identity_confidence, plan.title_confidence, plan.mode), (1.00, 1.00, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(
            plan.session,
            Some(SeasonSession {
                show_id: 99,
                season: 2,
                discs: vec!["FP".to_string()],
                assigned: vec![assigned_episode(1, "FP", 1), assigned_episode(2, "FP", 2), assigned_episode(3, "FP", 2)],
                episode_count: None,
                closed: false,
            })
        );
    }

    #[test]
    fn set_identity_attaches_an_unmapped_disc_to_its_season_session_without_episodes() {
        let disc = episodes_disc("SHOW_S2_D1", true);
        let name = disc_name(None, disc.volume_label.as_deref());
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let overrides = Overrides { identity: Some(show_id(99)), ..Overrides::default() };
        let sessions = [session_with(99, 2, None, "EARLIER", &[(1, 7), (2, 8), (3, 9)])];
        let lookups = LookupInputs { name: name.as_ref(), ..LookupInputs::default() };
        let mut planned_inputs = inputs(&disc, lookups, &settings, &library);
        planned_inputs.overrides = Some(&overrides);
        planned_inputs.fingerprint = Some("FP");
        planned_inputs.sessions = &sessions;
        let plan = run(&planned_inputs);
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Reviewed, 1.00));
        // The suggestions continue the session (8.5) and stay suggestions until review assigns them.
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, Some(2), &[4], None),
                suggested("t01-00002", 2, Some(2), &[5], None),
                suggested("t02-00003", 3, Some(2), &[6], None),
            ]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::TvUnmapped]);
        // Spec 11 Set identity "attaches the disc to a season session": the disc is recorded, no episode is assigned.
        let earlier = "EARLIER".to_string();
        assert_eq!(
            plan.session,
            Some(SeasonSession {
                show_id: 99,
                season: 2,
                discs: vec![earlier.clone(), "FP".to_string()],
                assigned: vec![
                    assigned_episode(1, &earlier, 7),
                    assigned_episode(2, &earlier, 8),
                    assigned_episode(3, &earlier, 9),
                ],
                episode_count: None,
                closed: false,
            })
        );
        assert!(plan
            .evidence
            .sentences
            .contains(&"the disc is attached to season session 99-s02.json until review assigns its episodes".to_string()));
    }

    #[test]
    fn the_reviewed_season_fills_planned_episodes_without_one() {
        let disc = episodes_disc("SHOW_D2", false);
        let name = disc_name(None, disc.volume_label.as_deref());
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let overrides = Overrides { season: Some(5), ..Overrides::default() };
        let lookups = LookupInputs { name: name.as_ref(), ..LookupInputs::default() };
        let mut planned_inputs = inputs(&disc, lookups, &settings, &library);
        planned_inputs.overrides = Some(&overrides);
        let plan = run(&planned_inputs);
        assert_eq!(
            plan.titles,
            vec![
                suggested("t00-00001", 1, Some(5), &[1], None),
                suggested("t01-00002", 2, Some(5), &[2], None),
                suggested("t02-00003", 3, Some(5), &[3], None),
            ]
        );
        // The season answers review's question; the episodes are still suggestions.
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown, ReviewReason::TvUnmapped]);
    }

    #[test]
    fn a_structural_feature_far_from_the_tmdb_runtime_is_held_at_0_60() {
        let disc = one_feature("30000000000");
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let overrides = Overrides { identity: Some(movie_id(6537)), ..Overrides::default() };

        // 8.3 rule 4: 100 minutes against TMDb's 120 is 16.7% off.
        let far = orphanage_record("en", Some(120));
        let lookups = LookupInputs { record: Some(&far), ..LookupInputs::default() };
        let mut planned_inputs = inputs(&disc, lookups, &settings, &library);
        planned_inputs.overrides = Some(&overrides);
        let plan = run(&planned_inputs);
        assert_eq!(plan.identity.step, IdentityStep::Reviewed);
        assert_eq!((plan.title_confidence, plan.mode), (0.60, Mode::Copy));
        assert_eq!(plan.reviews, vec![ReviewReason::TitleAmbiguous]);

        // 105 minutes: 4.8% off, within rule 4's 10%.
        let near = orphanage_record("en", Some(105));
        let lookups = LookupInputs { record: Some(&near), ..LookupInputs::default() };
        let mut planned_inputs = inputs(&disc, lookups, &settings, &library);
        planned_inputs.overrides = Some(&overrides);
        let plan = run(&planned_inputs);
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
    }

    #[test]
    fn the_space_margins_are_spec_7s() {
        assert_eq!((DIRECT_SPACE_MARGIN, COPY_SPACE_MARGIN, WRITE_SPACE_MARGIN), (0.05, 0.02, 0.05));
    }

    #[test]
    fn a_direct_rip_needs_the_planned_titles_sizes_plus_5_percent() {
        let one = pinned_log(JOHN_WICK_UHD, JOHN_WICK_UHD_API).plan();
        assert_eq!(one.mode, Mode::Direct);
        // 67,757,930,496 bytes times 1.05 is 71,145,827,020.8: rounded up. The disc size does not count in Direct mode.
        assert_eq!(staging_bytes_needed(&one, 0), 71_145_827_021);
        assert_eq!(staging_bytes_needed(&one, 99_999_999_999), 71_145_827_021);
        // Three episodes: 90,679,904,256 bytes times 1.05.
        let three = pinned_log(CHERNOBYL_1, CHERNOBYL_1_API).plan();
        assert_eq!(staging_bytes_needed(&three, 0), 95_213_899_469);
    }

    #[test]
    fn a_planned_title_without_a_size_counts_nothing() {
        let disc = one_feature("");
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&disc, LookupInputs::default(), &settings, &library));
        assert_eq!((plan.mode, plan.titles.len(), plan.titles[0].size), (Mode::Direct, 1, None));
        assert_eq!(staging_bytes_needed(&plan, 50_000_000_000), 0);
    }

    #[test]
    fn a_whole_disc_copy_needs_the_disc_size_plus_2_percent() {
        let disc = two_rivals();
        let (settings, library) = (Settings::default(), LibraryFacts::default());
        let plan = run(&inputs(&disc, LookupInputs::default(), &settings, &library));
        assert_eq!(plan.mode, Mode::Copy);
        // The 56.9 GB UHD of EXP-03: 56,900,000,000 bytes times 1.02; the planned titles do not count in copy mode.
        assert_eq!(staging_bytes_needed(&plan, 56_900_000_000), 58_038_000_000);
        assert_eq!(staging_bytes_needed(&plan, 101), 104);
        assert_eq!(staging_bytes_needed(&plan, 0), 0);
    }

    #[test]
    fn extraction_and_remux_need_the_files_plus_5_percent() {
        assert_eq!(write_bytes_needed(0), 0);
        assert_eq!(write_bytes_needed(1), 2);
        assert_eq!(write_bytes_needed(20), 21);
        assert_eq!(write_bytes_needed(100), 105);
        assert_eq!(write_bytes_needed(101), 107);
        assert_eq!(write_bytes_needed(u64::MAX), u64::MAX);
    }
}
