//! The decision document of one title, spec 6.4 step 11: "Decide tracks (8.6, 8.7) and disposition (8.8). A pure
//! function of the scan, the probes, the database answers and settings." `decide` runs the audio rule (8.6) and the
//! subtitle rule (8.7) with the owner's Override tracks (spec 11), lays the tracks out (8.10), builds the final path
//! (8.9, with spec 7's `path_collision` and spec 11's Resolve collision), fixes the three confidences of 8.8 and the
//! disposition; `same_path_reviews` is the job-level step that holds every title of a job whose final path another
//! title of the same job also builds (spec 7, 8.9's `path_collision`); `remember` is spec 11's Remember: a reviewed
//! job's decisions as a remembered-decisions file (spec 5).
//! Verification lowers no confidence: a failed or read-error rip holds the job through its review reason (6.2 step
//! 7, Open point 30). Written from the spec (RP20); used by `lazymkv-dev decide` (Task 16), the job reducer's
//! `DecisionDone` event (Task 14, `Disposition`) and phase 2's engine.

use crate::disc::{Disc, Title};
use crate::lang::wanted_languages;
use crate::lookup::tmdb::{TmdbIdentity, TmdbKind, TmdbRecord};
use crate::lookup::REMEMBERED_CONFIDENCE;
use crate::naming::{edition_label, movie_path, resolution_label, tv_path, NamedPath};
use crate::plan::movie::{edition_names, Feature, LibraryFacts};
use crate::plan::tv::Episode;
use crate::plan::{assigned_feature, Plan, PlannedRole, PlannedTitle};
use crate::remembered::{
    signature, AudioOverride, AudioOverrideEntry, Collision, Overrides, Remembered, RememberedTitle, StreamSignature,
    SubtitleOverride, SubtitleOverrideEntry, TitleRole,
};
use crate::review::{sorted_unique, Noted, ReviewReason};
use crate::settings::{configured_root, same_path, Settings};
use crate::tracks::audio::{decide_audio, AudioDecision, AudioFacts};
use crate::tracks::layout::{layout, Layout};
use crate::tracks::subtitle::{decide_subtitles, SubtitleDecision, SubtitleFacts};
use crate::verify::ReadError;

/// The three confidences of spec 8.8 for one title: identity and title as the plan fixed them at 6.1 step 5 (1.00
/// where the owner's Set identity or Assign titles covers them), and track, which "does not exist until the Decide
/// step": the lower of the audio and subtitle confidences (8.7). Spec 8.8. Def: T13. Use: T16.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Confidences {
    /// The plan's identity confidence (8.1), or 1.00 after the owner's Set identity.
    pub identity: f64,
    /// The plan's title confidence (8.3, 8.5), or 1.00 after the owner's Assign titles.
    pub title: f64,
    /// `track_confidence` of the audio and subtitle decisions.
    pub track: f64,
}

/// What 8.8 does with a decided job: file it automatically, or hold it for review with its rip or disc copy kept
/// (D1). Spec 8.8. Def: T13. Use: T14, T16.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// No review reason and every confidence at or above `auto_threshold`.
    AutoFile,
    /// Any review reason, or any confidence below `auto_threshold` (the job enters NeedsReview, 6.4 step 12).
    NeedsReview,
}

impl Disposition {
    /// Both values, in 8.8's order.
    pub const ALL: [Disposition; 2] = [Disposition::AutoFile, Disposition::NeedsReview];

    /// The serde spelling: "auto_file" or "needs_review".
    pub fn as_str(self) -> &'static str {
        match self {
            Disposition::AutoFile => "auto_file",
            Disposition::NeedsReview => "needs_review",
        }
    }
}

impl std::str::FromStr for Disposition {
    type Err = String;

    /// Only the two spellings of `as_str`.
    fn from_str(text: &str) -> Result<Disposition, String> {
        Disposition::ALL
            .into_iter()
            .find(|value| value.as_str() == text)
            .ok_or_else(|| format!("'{text}' is not a disposition (auto_file, needs_review)"))
    }
}

/// AutoFile only with no review reason and every confidence >= settings.auto_threshold (8.8: "A job files
/// automatically only if it has no review reason and all three confidences are at or above `auto_threshold`").
/// Reads settings.auto_threshold only. Def: T13. Use: T13, T16.
pub fn disposition(confidences: &Confidences, reviews: &[ReviewReason], settings: &Settings) -> Disposition {
    let threshold = settings.auto_threshold;
    let confident = [confidences.identity, confidences.title, confidences.track].into_iter().all(|value| value >= threshold);
    if reviews.is_empty() && confident {
        Disposition::AutoFile
    } else {
        Disposition::NeedsReview
    }
}

/// The lower of the two (8.7 "The job's track confidence is the lower of the audio and subtitle values"); each
/// already carries 1.00 where remembered or review overrides cover every one of its streams (Tasks 8 and 10).
/// Spec 8.7, 8.8. Def: T13. Use: T13.
pub fn track_confidence(audio: &AudioDecision, subtitles: &SubtitleDecision) -> f64 {
    audio.confidence.min(subtitles.confidence)
}

/// Everything 6.4 step 11 decides from: the plan of 6.1 step 5 with the planned title being decided and its scan
/// title, the facts the file phase measured (6.4 steps 9 to 10a), the TMDb record for naming, the library facts (8.4,
/// 7), the rip's review reasons and read errors (6.2 step 7, 6.3), the owner's review decisions (spec 11) and the
/// settings. Spec 6.4 step 11. Def: T13. Use: T16.
pub struct DecisionInputs<'a> {
    /// The job's plan: its identity and title confidences, reviews, notes, wanted languages and titles. After a wait in
    /// AwaitingLookup it is the refreshed plan (`plan::refresh_identity` over the answered lookups, which the reducer asks
    /// for with `job::Action::RefreshIdentity`), so the late TheDiscDB and TMDb answers reach the decision (8.1).
    pub plan: &'a Plan,
    /// The scan the plan was made from: the owner's Assign titles leaves out a title it lacks, as Task 7's re-plan
    /// (`assigned_entries`) does (Open point 32).
    pub disc: &'a Disc,
    /// The title being decided; one of `plan.titles`.
    pub planned: &'a PlannedTitle,
    /// The scan title of `planned` (`planned.title == title.number`).
    pub title: &'a Title,
    /// 8.6's facts (MD5s, ffprobe profiles, TheDiscDB's title, content verdicts, remembered overrides); `decide` sets
    /// `reviewed` to `overrides.audio_of(title.number)` when overrides are given.
    pub audio: AudioFacts<'a>,
    /// 8.7's facts (events, removed streams, TheDiscDB's title, text decisions, remembered overrides); `decide` sets
    /// `reviewed` to `overrides.subtitles_of(title.number)` when overrides are given.
    pub subtitles: SubtitleFacts<'a>,
    /// The TMDb record for naming (None: no path can be built; the reviews already hold identity_unknown or _ambiguous).
    pub record: Option<&'a TmdbRecord>,
    /// 8.4's library facts and spec 7's path collision, read by phase 2 (RP18).
    pub library: &'a LibraryFacts,
    /// 6.2 step 7's outcome for this title's rip (verify_failed, read_errors) and 6.3's copy_hash_errors, carried in.
    pub rip_reviews: Vec<ReviewReason>,
    /// The rip's VerifyReport.read_errors (6.2 step 7 "with the error count and offsets").
    pub read_errors: Vec<ReadError>,
    /// The owner's review decisions (spec 11) at the disposition gate or on re-finalise.
    pub overrides: Option<&'a Overrides>,
    /// Spec 14; `decide` reads auto_threshold, movies_root and tv_root, and hands the rest to the rules it calls.
    pub settings: &'a Settings,
}

/// One title's file-phase decision (6.4 step 11). Not `Decision`, which is `subtitle::text::Decision`. Field order is
/// the document's key order. Def: T13. Use: T16.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileDecision {
    /// The planned title's key (spec 5).
    pub title_key: String,
    /// 8.6's decision, the owner's Override tracks applied.
    pub audio: AudioDecision,
    /// 8.7's decision, the owner's Override tracks applied. Its tracks' `default` stays false: the file's default
    /// subtitle is `layout`'s (8.10, `tracks::layout::default_subtitle`; the shared interfaces' doc of the field).
    pub subtitles: SubtitleDecision,
    /// 8.10's tracks of the final file.
    pub layout: Layout,
    /// 8.9's resolution label of the first video stream's height; None without a video height.
    pub resolution: Option<String>,
    /// 8.4's edition name as filed (through `edition_label`); None for an episode or an unnamed cut.
    pub edition: Option<String>,
    /// 8.9's final path; None when it cannot be built (no record, identity, root or video height, a record of the
    /// other kind than the role's, or an episode without season or episode number).
    pub path: Option<NamedPath>,
    pub confidences: Confidences,
    /// Sorted and unique (`review::sorted_unique`).
    pub reviews: Vec<ReviewReason>,
    /// The plan's notes, then the audio decision's, the subtitle decision's and the naming notes, in that order.
    pub notes: Vec<Noted>,
    /// The rip's read errors, carried unchanged (6.2 step 7).
    pub read_errors: Vec<ReadError>,
    pub disposition: Disposition,
}

/// Spec 11's Resolve collision "keep both": the new file's distinguishing label (`- 2160p 2`).
const KEEP_BOTH_COPY: u32 = 2;

/// 6.4 step 11 and 8.8: decide_audio (with the wanted languages of the identity the job is filed under: whenever that
/// identity's TMDb record is given, `wanted_languages` of the record's original language, so a record fetched after
/// planning (a lookup that was pending, or the owner's Set identity) gives its original language; the plan's without a
/// record; R4, 8.6 "Languages") and decide_subtitles (facts with `reviewed` set from
/// `overrides.audio_of(title.number)` and `overrides.subtitles_of(title.number)`), layout, the path (movie_path for a
/// feature, tv_path for an episode, by the filed role: the owner's Assign titles when it names any title, which then wins
/// whole as in a re-plan (Err for a planned title it ignores or does not name), else the planned role; no path when the
/// record's kind does not match the role (a movie record for a feature, a show record for an episode); resolution from
/// title.video_height, the edition through edition_names and edition_label; a planned Episode with season None builds no
/// path (path None) and adds tv_unmapped;
/// a Resolve collision answer (`overrides.collision`) for the decision's own plain final path, KeepBoth (copy Some(2)
/// while the collision exists) or Replace, leaves path_collision unraised, and an answer for any other path resolves
/// nothing (spec 7); Discard never reaches decide, the engine discards the job), the three confidences (identity and
/// title from the plan; identity 1.00 when overrides.identity is Some, title 1.00 when overrides.titles is not empty; track
/// = min(audio, subtitle) after the remembered and review overrides set the parts they cover to 1.00), reviews = plan's
/// (less the reasons the owner's review answers: identity_unknown and identity_ambiguous when overrides.identity is Some;
/// title_ambiguous, multiple_features, unnamed_cut and tv_unmapped when overrides.titles is not empty) + audio's +
/// subtitles' + rip_reviews + path_collision when library.path_collision (and no KeepBoth or Replace answer names the
/// decision's plain final path, or no path is built) + read_errors
/// exactly when `read_errors` is not empty, notes likewise, read_errors carried unchanged, disposition; NeedsReview
/// whenever path is None. Verification lowers no
/// confidence: a failed or read-error rip holds the job through its review reason, which has the same effect on the
/// disposition as a confidence below auto_threshold (8.8). The owner's Assign titles, when it names any title, gives
/// the role the title is filed under exactly as Task 7's re-plan applies it (Open point 32), and that role chooses
/// movie_path or tv_path; a decision without a path is NeedsReview (a job with no final path cannot be filed; Open point
/// 38). Err for a planned title that is not in the plan or not the scan title given, a title the owner ignored or left
/// out of an Assign titles, a discarded job, or a record of another identity than the job's. Def: T13. Use: T16.
pub fn decide(inputs: &DecisionInputs<'_>) -> Result<FileDecision, String> {
    let plan = inputs.plan;
    let planned = inputs.planned;
    let title = inputs.title;
    let settings = inputs.settings;
    let overrides = inputs.overrides;
    if !plan.titles.iter().any(|entry| entry == planned) {
        return Err(format!("the plan holds no title {} with key {}", planned.title, planned.key));
    }
    if planned.title != title.number {
        return Err(format!(
            "planned title {} ({}) cannot be decided from scan title {}",
            planned.title, planned.key, title.number
        ));
    }
    if overrides.is_some_and(Overrides::discards) {
        return Err(format!(
            "the owner discarded the job (spec 11 Resolve collision), so title {} ({}) is not decided",
            planned.title, planned.key
        ));
    }
    let role = filed_role(planned, overrides)?;
    let identity = overrides.and_then(|given| given.identity).or(plan.identity.tmdb);
    let record = naming_record(inputs.record, identity)?;

    // 8.6 and 8.7, with the owner's Override tracks for this title (spec 11; Overrides holds them as entries keyed by
    // title and stream, and `audio_of` and `subtitles_of` give one title's by stream), which win over the remembered ones.
    let reviewed_audio = overrides.and_then(|given| given.audio_of(title.number));
    let reviewed_subtitles = overrides.and_then(|given| given.subtitles_of(title.number));
    let mut audio_facts = inputs.audio.clone();
    let mut subtitle_facts = inputs.subtitles.clone();
    if overrides.is_some() {
        audio_facts.reviewed = reviewed_audio.as_ref();
        subtitle_facts.reviewed = reviewed_subtitles.as_ref();
    }
    // R4, 8.6 "Languages": the wanted languages of the identity the job is filed under, from its record's original
    // language whenever the record is given. The plan's were computed at planning, when the record may not have been in
    // hand (a pending TMDb lookup gives English only) or named another work (the owner's Set identity); for a record
    // present at planning this is the same set (a re-plan with the same overrides computes it too, Open point 32).
    let wanted = match record {
        Some(record) => wanted_languages(record.original_language.as_deref()),
        None => plan.wanted.clone(),
    };
    let audio = decide_audio(title, &wanted, &audio_facts, settings);
    let subtitles = decide_subtitles(title, &subtitle_facts, settings);
    let track_layout = layout(title, &audio, &subtitles);

    // 8.9: the resolution label, the edition (8.4), the final path; spec 11's keep both adds the copy label while the
    // collision exists (with the colliding file gone, the plain path is free).
    let resolution = title.video_height().map(|height| resolution_label(height).to_string());
    let edition = match &role {
        PlannedRole::Feature { .. } => filed_edition(plan, inputs.disc, planned.title, overrides, inputs.library),
        PlannedRole::Episode(_) => None,
    };
    // Spec 11's Resolve collision answers the plain final path the owner was shown: only an answer for this decision's
    // own plain path resolves its collision (spec 7).
    let plain = filing(&role, record, resolution.as_deref(), edition.as_deref(), None, settings);
    let answered_collision = match (&plain.path, overrides) {
        (Some(named), Some(given)) => given.collision_for(&named.path),
        _ => None,
    };
    let Filing { path, notes: path_notes, unmapped } =
        if answered_collision == Some(Collision::KeepBoth) && inputs.library.path_collision {
            filing(&role, record, resolution.as_deref(), edition.as_deref(), Some(KEEP_BOTH_COPY), settings)
        } else {
            plain
        };

    // 8.8: identity and title as planned, 1.00 where the owner's review covers them; track the lower of the two rules.
    let identity_reviewed = overrides.map_or(false, |given| given.identity.is_some());
    let titles_reviewed = overrides.map_or(false, |given| !given.titles.is_empty());
    let confidences = Confidences {
        identity: if identity_reviewed { REMEMBERED_CONFIDENCE } else { plan.identity_confidence },
        title: if titles_reviewed { REMEMBERED_CONFIDENCE } else { plan.title_confidence },
        track: track_confidence(&audio, &subtitles),
    };

    // 8.8's review reasons: the plan's (less those the owner's Set identity or Assign titles answers), the two rules',
    // the rip's; read_errors exactly when the rip logged some.
    let answered = |reason: &ReviewReason| {
        (identity_reviewed && matches!(reason, ReviewReason::IdentityUnknown | ReviewReason::IdentityAmbiguous))
            || (titles_reviewed
                && matches!(
                    reason,
                    ReviewReason::TitleAmbiguous
                        | ReviewReason::MultipleFeatures
                        | ReviewReason::UnnamedCut
                        | ReviewReason::TvUnmapped
                ))
    };
    let mut reasons: Vec<ReviewReason> = plan
        .reviews
        .iter()
        .filter(|reason| !answered(*reason))
        .chain(&audio.reviews)
        .chain(&subtitles.reviews)
        .chain(&inputs.rip_reviews)
        .copied()
        .filter(|reason| *reason != ReviewReason::ReadErrors)
        .collect();
    if unmapped {
        reasons.push(ReviewReason::TvUnmapped);
    }
    let resolved = matches!(answered_collision, Some(Collision::KeepBoth) | Some(Collision::Replace));
    if inputs.library.path_collision && !resolved {
        reasons.push(ReviewReason::PathCollision);
    }
    if !inputs.read_errors.is_empty() {
        reasons.push(ReviewReason::ReadErrors);
    }
    let reviews = sorted_unique(reasons);

    let mut notes: Vec<Noted> = plan.notes.iter().chain(&audio.notes).chain(&subtitles.notes).cloned().collect();
    notes.extend(path_notes);

    // 8.8's disposition; a job with no final path cannot be filed (Open point 38).
    let outcome = if path.is_some() { disposition(&confidences, &reviews, settings) } else { Disposition::NeedsReview };
    Ok(FileDecision {
        title_key: planned.key.clone(),
        audio,
        subtitles,
        layout: track_layout,
        resolution,
        edition,
        path,
        confidences,
        reviews,
        notes,
        read_errors: inputs.read_errors.clone(),
        disposition: outcome,
    })
}

/// The job-level step after every title of one job is decided (6.4 step 11, before the disposition gate of 6.4 step
/// 12): each decision whose final path another decision of the same job also builds gets `path_collision` (its reviews
/// sorted and unique again) and NeedsReview. Paths are compared whole and case-insensitively, separators and a trailing
/// separator aside (`settings::same_path`), since the library roots are Windows shares. Applied whatever
/// `overrides.collision` says: spec 11's Keep both and Replace resolve a collision with a file already in the library,
/// never two titles of one job; the owner settles a same-job clash with Assign titles (naming the cuts, D2, 8.4), and
/// the job reducer keeps a job held while `path_collision` is present, approved or not. A decision without a path is
/// left as it is (it is NeedsReview already, Open point 38). Covers every way two titles reach one path: two assigned
/// features or a Feature beside a "Theatrical Cut", structural rivals approved without names, two cut names that share
/// their first 32 characters, two titles planned as one episode. Spec 7 ("Path collisions add `path_collision`"), 8.9
/// ("A second copy at the same resolution and edition is a `path_collision`"), D1. Def: final review (C3, G3). Use:
/// `lazymkv-dev decide`, phase 2 (before the job's `DecisionDone`, `job::decision_done`).
pub fn same_path_reviews(decisions: &mut [FileDecision]) {
    let paths: Vec<Option<String>> =
        decisions.iter().map(|decision| decision.path.as_ref().map(|named| named.path.clone())).collect();
    for (index, decision) in decisions.iter_mut().enumerate() {
        let Some(path) = paths[index].as_deref() else {
            continue;
        };
        let shared = paths
            .iter()
            .enumerate()
            .any(|(other, found)| other != index && found.as_deref().is_some_and(|found| same_path(found, path)));
        if shared {
            let mut reviews = std::mem::take(&mut decision.reviews);
            reviews.push(ReviewReason::PathCollision);
            decision.reviews = sorted_unique(reviews);
            decision.disposition = Disposition::NeedsReview;
        }
    }
}

/// What the naming step gave: the final path when it could be built, the naming notes (8.9's compat_tag_omitted),
/// and whether an episode lacked its season or its episode numbers (8.5's tv_unmapped).
struct Filing {
    path: Option<NamedPath>,
    notes: Vec<Noted>,
    unmapped: bool,
}

/// The role the planned title is filed under (Open point 32). The owner's Assign titles, when it names any title, wins
/// whole, as Task 7's re-plan applies it (`assigned_roles`): the title's assignment, with Err for a title it ignores and
/// for a planned title it does not name (nothing is filed for either). Without it, the plan's role. A planned episode
/// without a season takes the owner's season (8.5 "review asks for it").
fn filed_role(planned: &PlannedTitle, overrides: Option<&Overrides>) -> Result<PlannedRole, String> {
    let assigned = overrides.filter(|given| !given.titles.is_empty());
    let mut role = match assigned {
        None => planned.role.clone(),
        Some(assigned) => {
            let given = assigned.title_role(planned.title).ok_or_else(|| {
                format!(
                    "the owner's Assign titles does not name title {} ({}), so nothing is filed for it",
                    planned.title, planned.key
                )
            })?;
            assigned_role(planned.title, given).ok_or_else(|| {
                format!(
                    "the owner's Assign titles ignores title {} ({}), so nothing is filed for it",
                    planned.title, planned.key
                )
            })?
        }
    };
    if let PlannedRole::Episode(episode) = &mut role {
        if episode.season.is_none() {
            episode.season = overrides.and_then(|given| given.season);
        }
    }
    Ok(role)
}

/// Spec 11's Assign titles as a planned role: a feature or named cut through `plan::assigned_feature` (the one mapping,
/// which Task 7's re-plan also calls), or episodes (with no name of their own, as Task 7's `assigned_entries` builds
/// them; `episode_name` then takes TMDb's); None for Ignored.
fn assigned_role(number: u32, role: &TitleRole) -> Option<PlannedRole> {
    if let Some(feature) = assigned_feature(number, role) {
        return Some(PlannedRole::Feature { edition: feature.edition, theatrical: feature.theatrical });
    }
    match role {
        TitleRole::Episodes { season, episodes } => {
            Some(PlannedRole::Episode(Episode { season: Some(*season), episodes: episodes.clone(), name: None }))
        }
        TitleRole::Feature | TitleRole::Cut { .. } | TitleRole::Ignored => None,
    }
}

/// The record naming may use (8.9 "Title and year always come from the same TMDb record as the ID"): the given record
/// when it is the identity's; None without a record or without an identity; Err for a record of another identity.
fn naming_record<'r>(
    record: Option<&'r TmdbRecord>,
    identity: Option<TmdbIdentity>,
) -> Result<Option<&'r TmdbRecord>, String> {
    match (record, identity) {
        (Some(record), Some(identity)) if record.identity == identity => Ok(Some(record)),
        (Some(record), Some(identity)) => Err(format!(
            "the TMDb record is {} {} but the job's identity is {} {}: title and year come from the identity's own \
             record (spec 8.9)",
            record.identity.kind.as_str(),
            record.identity.id,
            identity.kind.as_str(),
            identity.id
        )),
        _ => Ok(None),
    }
}

/// 8.4's edition name of feature `number` as filed: `edition_names` over the job's features with the library facts of
/// decide time, then `edition_label` (sanitised, 32 characters; an empty label is no edition).
fn filed_edition(
    plan: &Plan,
    disc: &Disc,
    number: u32,
    overrides: Option<&Overrides>,
    library: &LibraryFacts,
) -> Option<String> {
    let features = job_features(plan, disc, overrides);
    let index = features.iter().position(|feature| feature.title == number)?;
    let name = edition_names(&features, library).into_iter().nth(index).flatten()?;
    let label = edition_label(&name);
    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

/// The job's features and cuts for 8.4 (choice 2). The owner's Assign titles, when it names any title, wins whole: its
/// features and cuts in title order through `plan::assigned_feature`, as Task 7's `assigned_entries` builds them. Else the
/// plan's features in plan order, each with Task 7's filed name and the planner's theatrical flag
/// (`PlannedRole::Feature.theatrical`): an unnamed cut (8.3 rule 2's Other suffix) or a structural rival is not
/// theatrical, so it keeps no edition until the owner names it (8.4), even as the plan's only feature. An assigned title
/// the scan lacks is left out, as Task 7's `assigned_entries` leaves it out (Open point 32).
fn job_features(plan: &Plan, disc: &Disc, overrides: Option<&Overrides>) -> Vec<Feature> {
    if let Some(titles) = overrides.map(|given| &given.titles).filter(|titles| !titles.is_empty()) {
        // A title the scan lacks is left out, as Task 7's assigned_entries leaves it out (Open point 32).
        return titles
            .iter()
            .filter(|entry| disc.titles.contains_key(&entry.title))
            .filter_map(|entry| assigned_feature(entry.title, &entry.role))
            .collect();
    }
    plan.titles
        .iter()
        .filter_map(|entry| match &entry.role {
            PlannedRole::Feature { edition, theatrical } => Some(Feature {
                title: entry.title,
                edition: edition.clone(),
                theatrical: *theatrical,
                siblings: Vec::new(),
            }),
            PlannedRole::Episode(_) => None,
        })
        .collect()
}

/// The final path of 8.9 for the role: a feature through `movie_path` under settings.movies_root (needs a movie record
/// and a resolution), an episode through `tv_path` under settings.tv_root (needs a show record, a season and an episode
/// number; without the last two it is unmapped, 8.5). A blank root (no path component, `settings::configured_root`) is
/// a root not set: no path is built, so the job is held rather than filed relative to the engine's working folder. A record of the other kind builds no path: TMDb's two kinds number
/// their records separately (8.1), so a movie's ID would name a show folder, or a show's ID a movie folder (choice 1).
fn filing(
    role: &PlannedRole,
    record: Option<&TmdbRecord>,
    resolution: Option<&str>,
    edition: Option<&str>,
    copy: Option<u32>,
    settings: &Settings,
) -> Filing {
    let unbuilt = |unmapped: bool| Filing { path: None, notes: Vec::new(), unmapped };
    match role {
        PlannedRole::Feature { .. } => match (record, configured_root(settings.movies_root.as_deref()), resolution) {
            (Some(record), Some(root), Some(resolution)) if record.identity.kind == TmdbKind::Movie => {
                let (path, notes) = movie_path(root, &record.work(), resolution, edition, copy, settings);
                Filing { path: Some(path), notes, unmapped: false }
            }
            _ => unbuilt(false),
        },
        PlannedRole::Episode(episode) => {
            let season = match episode.season {
                Some(season) if !episode.episodes.is_empty() => season,
                _ => return unbuilt(true),
            };
            match (record, configured_root(settings.tv_root.as_deref())) {
                (Some(record), Some(root)) if record.identity.kind == TmdbKind::Tv => {
                    let name = episode_name(episode, season, record);
                    let (path, notes) =
                        tv_path(root, &record.work(), season, &episode.episodes, name.as_deref(), copy, settings);
                    Filing { path: Some(path), notes, unmapped: false }
                }
                _ => unbuilt(false),
            }
        }
    }
}

/// The episode title of 8.9: the plan's (TheDiscDB's item title), else TMDb's name of the episode when the title holds
/// exactly one episode; none for a multi-episode title without a name of its own.
fn episode_name(episode: &Episode, season: u32, record: &TmdbRecord) -> Option<String> {
    if episode.name.is_some() {
        return episode.name.clone();
    }
    let [only] = episode.episodes.as_slice() else {
        return None;
    };
    record.season(season)?.episodes.iter().find(|candidate| candidate.episode_number == *only)?.name.clone()
}

/// Spec 11 "Remember": the decisions of a reviewed job as a remembered-decisions file: the identity
/// (overrides.identity, else plan.identity.tmdb; Err without one), the titles as RememberedTitles (source file, attribute
/// 24, segment map, seconds, role, and the title's own track overrides): the owner's Assign titles whole, in title order
/// (Ignored included, a title the scan lacks left out), when it names any title, else every planned title under its
/// planned role (Err for an episode without a season or episode numbers, and for a feature without an edition that the
/// planner did not flag theatrical: an unnamed cut or a structural rival the owner must name first, 8.4, even as the
/// plan's only feature). The owner's Assign titles is stored as the decision and a re-plan apply it (whole: a planned
/// title it does not name is not remembered, and a title it names that the scan lacks is left out); an unnamed feature
/// is not `TitleRole::Feature`, which means the theatrical or main cut and would come back as the theatrical cut. Each
/// decision's kept and dropped audio and subtitle streams become override entries of its own title, keyed by
/// `remembered::signature` within that title, so two titles that decide streams of one signature differently each keep
/// their own decision (spec 5: "for each title ... its audio and subtitle overrides"): a kept audio track is `Keep` with
/// its 0-based final position and its default flag, a dropped one `Drop` (or `Commentary` when the owner marked it so);
/// a kept subtitle track is `Keep` with its role, a dropped one `Drop`; kept tracks first in final order, then the
/// dropped ones. A remembered title without a decision (Ignored) has no override. Err for a job without an identity, a
/// planned episode without a season or episode numbers, an unnamed planned cut, a title without a duration, a decision
/// whose key is not in the plan or whose title the remembered titles do not hold, or a stream its scan title lacks.
/// Spec 5, 11. Def: T13. Use: phase 2.
pub fn remember(
    fingerprint: &str,
    disc: &Disc,
    plan: &Plan,
    decisions: &[FileDecision],
    overrides: &Overrides,
) -> Result<Remembered, String> {
    let tmdb = overrides.identity.or(plan.identity.tmdb).ok_or_else(|| {
        "the job has no TMDb identity to remember: set its identity first (spec 11 Remember)".to_string()
    })?;

    // The titles (spec 5): the owner's Assign titles, when it names any title, whole and in title order (Ignored
    // included), as the decision and a re-plan apply it; else every planned title under its planned role. Each is kept
    // with its scan title number, so the decisions' overrides join their own title.
    let mut titles: Vec<(u32, RememberedTitle)> = Vec::new();
    if overrides.titles.is_empty() {
        for planned in &plan.titles {
            let role = stored_role(&planned.role, overrides.season).ok_or_else(|| match &planned.role {
                PlannedRole::Feature { .. } => format!(
                    "planned title {} ({}) is a cut without an edition name to remember: name it first (spec 8.4)",
                    planned.title, planned.key
                ),
                PlannedRole::Episode(_) => format!(
                    "planned title {} ({}) has no season and episode numbers to remember: assign them first (spec 8.5)",
                    planned.title, planned.key
                ),
            })?;
            titles.push((planned.title, remembered_title(disc.title(planned.title)?, role)?));
        }
    } else {
        // A title the scan lacks is left out, as the decision and Task 7's re-plan leave it out (choice 9).
        for assigned in overrides.titles.iter().filter(|assigned| disc.titles.contains_key(&assigned.title)) {
            titles.push((assigned.title, remembered_title(disc.title(assigned.title)?, assigned.role.clone())?));
        }
    }

    // Each decision's actions, keyed by signature within its own title (spec 5).
    for decision in decisions {
        let planned = plan
            .titles
            .iter()
            .find(|entry| entry.key == decision.title_key)
            .ok_or_else(|| format!("decision {} is not a title of the plan", decision.title_key))?;
        let title = disc.title(planned.title)?;
        let stored = titles
            .iter_mut()
            .find(|(number, _)| *number == planned.title)
            .map(|(_, stored)| stored)
            .ok_or_else(|| {
                format!(
                    "decision {} is for title {}, which the remembered titles do not hold",
                    decision.title_key, planned.title
                )
            })?;
        let reviewed_audio = overrides.audio_of(planned.title).unwrap_or_default();
        for (order, track) in decision.audio.kept.iter().enumerate() {
            let action = AudioOverride::Keep { order: order as u32, default: track.default };
            stored.audio.push(AudioOverrideEntry { signature: stream_signature(title, track.stream)?, action });
        }
        for (stream, _) in &decision.audio.dropped {
            let action = match reviewed_audio.get(stream) {
                Some(AudioOverride::Commentary) => AudioOverride::Commentary,
                _ => AudioOverride::Drop,
            };
            stored.audio.push(AudioOverrideEntry { signature: stream_signature(title, *stream)?, action });
        }
        for track in &decision.subtitles.kept {
            let action = SubtitleOverride::Keep { role: track.role };
            stored.subtitles.push(SubtitleOverrideEntry { signature: stream_signature(title, track.stream)?, action });
        }
        for (stream, _) in &decision.subtitles.dropped {
            let signature = stream_signature(title, *stream)?;
            stored.subtitles.push(SubtitleOverrideEntry { signature, action: SubtitleOverride::Drop });
        }
    }
    Ok(Remembered {
        fingerprint: fingerprint.to_string(),
        tmdb,
        titles: titles.into_iter().map(|(_, stored)| stored).collect(),
    })
}

/// A planned role as spec 5 stores it: the theatrical or main feature (`TitleRole::Feature`, a feature without an
/// edition the planner flagged theatrical), a cut with its edition name ("Theatrical Cut" included), or the episodes with
/// the episode's season, else the owner's season. None for a feature without an edition the planner did not flag
/// theatrical (an unnamed cut or a structural rival, 8.3 and 8.4: `TitleRole::Feature` means the theatrical or main
/// cut, and Task 7 would plan it back as the theatrical cut), and for an episode without a season or without an episode
/// number.
fn stored_role(role: &PlannedRole, season: Option<u32>) -> Option<TitleRole> {
    match role {
        PlannedRole::Feature { edition: None, theatrical: false } => None,
        PlannedRole::Feature { edition: None, theatrical: true } => Some(TitleRole::Feature),
        PlannedRole::Feature { edition: Some(edition), .. } => Some(TitleRole::Cut { edition: edition.clone() }),
        PlannedRole::Episode(episode) => {
            let season = episode.season.or(season)?;
            if episode.episodes.is_empty() {
                return None;
            }
            Some(TitleRole::Episodes { season, episodes: episode.episodes.clone() })
        }
    }
}

/// Spec 5's "for each title its source file, segment map, duration, role", with attribute 24 for a DVD (Review Focus 1).
fn remembered_title(title: &Title, role: TitleRole) -> Result<RememberedTitle, String> {
    let seconds =
        title.seconds.ok_or_else(|| format!("title {} has no duration (attribute 9) to remember", title.number))?;
    Ok(RememberedTitle {
        source_file: title.source_file.clone(),
        dvd_title: title.dvd_title.clone(),
        segment_map: title.segment_map.clone(),
        seconds,
        role,
        audio: Vec::new(),
        subtitles: Vec::new(),
    })
}

/// The signature of the title's stream `stream` (a scan stream number), through `remembered::signature`.
fn stream_signature(title: &Title, stream: u32) -> Result<StreamSignature, String> {
    let index = title
        .streams
        .iter()
        .position(|candidate| candidate.number == stream)
        .ok_or_else(|| format!("title {} has no stream {}", title.number, stream))?;
    Ok(signature(&title.streams, index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use serde_json::{json, Value};

    use crate::codec::Family;
    use crate::disc::{disc, StreamKind};
    use crate::lookup::tmdb::{TmdbEpisode, TmdbSeason};
    use crate::lookup::{Identity, IdentityStep, Kind, RuntimeFacts};
    use crate::plan::movie::{is_theatrical, TitleRule};
    use crate::plan::{title_key, KindSource, Mode, PlanEvidence};
    use crate::remembered::{
        apply_audio, apply_subtitles, apply_titles, matched_titles, AudioTrackOverride, CollisionAnswer,
        SubtitleTrackOverride, TitleOverrideEntry,
    };
    use crate::review::Note;
    use crate::robot::parse;
    use crate::settings::EditionStyle;
    use crate::testsupport::scan_text;
    use crate::tracks::audio::AudioDrop;
    use crate::tracks::subtitle::{SubtitleCase, SubtitleDrop};
    use crate::tracks::SubtitleRole;

    /// An audio decision that keeps nothing, at `confidence` (only the confidence reaches the track confidence).
    fn audio_at(confidence: f64) -> AudioDecision {
        AudioDecision {
            kept: Vec::new(),
            dropped: Vec::new(),
            reference: None,
            verdict_reference: None,
            notes: Vec::new(),
            reviews: Vec::new(),
            confidence,
            description_alignment: None,
        }
    }

    /// A subtitle decision that keeps nothing, in `case` at `confidence`.
    fn subtitles_at(case: SubtitleCase, confidence: f64) -> SubtitleDecision {
        SubtitleDecision {
            kept: Vec::new(),
            dropped: Vec::new(),
            set_aside: Vec::new(),
            full: Vec::new(),
            groups: Vec::new(),
            dialogue_group: None,
            dialogue_pick: None,
            pairs: Vec::new(),
            case,
            confidence,
            notes: Vec::new(),
            reviews: Vec::new(),
        }
    }

    fn at(identity: f64, title: f64, track: f64) -> Confidences {
        Confidences { identity, title, track }
    }

    #[test]
    fn the_track_confidence_is_the_subtitles_090_when_one_full_english_track_is_below_the_audios_095() {
        assert_eq!(track_confidence(&audio_at(0.95), &subtitles_at(SubtitleCase::OneFull, 0.90)), 0.90);
    }

    #[test]
    fn the_track_confidence_is_the_subtitles_085_when_alignment_decided_between_full_tracks() {
        assert_eq!(track_confidence(&audio_at(0.95), &subtitles_at(SubtitleCase::Alignment, 0.85)), 0.85);
    }

    #[test]
    fn the_track_confidence_is_the_subtitles_060_when_subtitle_ambiguous_was_raised() {
        assert_eq!(track_confidence(&audio_at(0.95), &subtitles_at(SubtitleCase::Ambiguous, 0.60)), 0.60);
    }

    #[test]
    fn the_track_confidence_is_095_when_flags_or_descriptions_decided_every_subtitle_track() {
        assert_eq!(track_confidence(&audio_at(0.95), &subtitles_at(SubtitleCase::FlagsOrDescriptions, 0.95)), 0.95);
    }

    #[test]
    fn the_track_confidence_is_095_when_the_title_has_no_english_subtitle_stream() {
        assert_eq!(track_confidence(&audio_at(0.95), &subtitles_at(SubtitleCase::NoEnglish, 0.95)), 0.95);
    }

    #[test]
    fn the_track_confidence_is_the_audios_095_when_every_subtitle_stream_is_overridden() {
        assert_eq!(track_confidence(&audio_at(0.95), &subtitles_at(SubtitleCase::OneFull, 1.00)), 0.95);
    }

    #[test]
    fn the_track_confidence_is_100_only_when_audio_and_subtitles_are_both_covered_by_overrides() {
        assert_eq!(track_confidence(&audio_at(1.00), &subtitles_at(SubtitleCase::OneFull, 1.00)), 1.00);
        assert_eq!(track_confidence(&audio_at(1.00), &subtitles_at(SubtitleCase::OneFull, 0.90)), 0.90);
    }

    #[test]
    fn a_job_with_no_review_reason_and_every_confidence_at_the_default_085_files_automatically() {
        let settings = Settings::default();
        assert_eq!(settings.auto_threshold, 0.85);
        assert_eq!(disposition(&at(0.85, 0.85, 0.85), &[], &settings), Disposition::AutoFile);
        assert_eq!(disposition(&at(1.00, 0.98, 0.95), &[], &settings), Disposition::AutoFile);
    }

    #[test]
    fn an_identity_confidence_below_085_holds_the_job() {
        assert_eq!(disposition(&at(0.84, 0.98, 0.95), &[], &Settings::default()), Disposition::NeedsReview);
    }

    #[test]
    fn a_title_confidence_below_085_holds_the_job() {
        assert_eq!(disposition(&at(0.95, 0.84, 0.95), &[], &Settings::default()), Disposition::NeedsReview);
    }

    #[test]
    fn a_track_confidence_below_085_holds_the_job() {
        assert_eq!(disposition(&at(0.95, 0.98, 0.84), &[], &Settings::default()), Disposition::NeedsReview);
    }

    #[test]
    fn every_review_reason_holds_the_job_whatever_its_confidences() {
        let settings = Settings::default();
        for reason in ReviewReason::ALL {
            assert_eq!(
                disposition(&at(1.00, 1.00, 1.00), &[reason], &settings),
                Disposition::NeedsReview,
                "{} must hold the job (8.8)",
                reason.as_str()
            );
        }
    }

    #[test]
    fn an_auto_threshold_of_095_holds_a_job_at_090_and_files_one_at_095() {
        let settings = Settings { auto_threshold: 0.95, ..Settings::default() };
        assert_eq!(disposition(&at(0.95, 0.98, 0.90), &[], &settings), Disposition::NeedsReview);
        assert_eq!(disposition(&at(0.95, 0.98, 0.95), &[], &settings), Disposition::AutoFile);
    }

    #[test]
    fn confidences_and_dispositions_serialise_with_the_spec_spellings() {
        let value = serde_json::to_value(at(0.95, 0.98, 0.90)).unwrap();
        assert_eq!(value, json!({"identity": 0.95, "title": 0.98, "track": 0.90}));
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["identity", "title", "track"]);
        assert_eq!(serde_json::to_value(Disposition::AutoFile).unwrap(), json!("auto_file"));
        assert_eq!(serde_json::to_value(Disposition::NeedsReview).unwrap(), json!("needs_review"));
        assert_eq!(Disposition::AutoFile.as_str(), "auto_file");
        assert_eq!(Disposition::NeedsReview.as_str(), "needs_review");
        assert_eq!(Disposition::ALL, [Disposition::AutoFile, Disposition::NeedsReview]);
        let back: Disposition = serde_json::from_value(json!("needs_review")).unwrap();
        assert_eq!(back, Disposition::NeedsReview);
    }

    #[test]
    fn a_disposition_parses_from_its_spelling_and_from_nothing_else() {
        for value in Disposition::ALL {
            assert_eq!(value.as_str().parse::<Disposition>(), Ok(value));
        }
        assert_eq!(
            "AutoFile".parse::<Disposition>(),
            Err("'AutoFile' is not a disposition (auto_file, needs_review)".to_string())
        );
        assert!("held".parse::<Disposition>().is_err());
        assert!("".parse::<Disposition>().is_err());
    }

    const MOVIES: &str = r"\\nas\media\Movies";
    const SHOWS: &str = r"\\nas\media\Shows";
    const FILM: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 12345 };
    const OFFICE: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Tv, id: 2316 };
    const SCRAMBLED: &str = "Scsi error - ILLEGAL REQUEST:READ OF SCRAMBLED SECTOR WITHOUT AUTHENTICATION";

    const UHD_VIDEO: &[(u32, &str)] =
        &[(1, "Video"), (5, "V_MPEGH/ISO/HEVC"), (6, "MpegH"), (7, "MpegH HEVC Main10@L5.1"), (19, "3840x2160")];
    const HD_VIDEO: &[(u32, &str)] =
        &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1"), (19, "1920x1080")];
    const DVD_VIDEO: &[(u32, &str)] = &[(1, "Video"), (5, "V_MPEG2"), (6, "Mpeg2"), (7, "Mpeg2"), (19, "720x480")];
    const ENG_DTS_HD_MA_6CH: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_DTS"), (6, "DTS-HD MA"), (7, "DTS-HD Master Audio"), (14, "6")];
    const ENG_AC3_6CH: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (13, "640 Kb/s"), (14, "6")];
    const SPA_AC3_2CH: &[(u32, &str)] =
        &[(1, "Audio"), (3, "spa"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (13, "192 Kb/s"), (14, "2")];
    const ENG_PGS: &[(u32, &str)] = &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];
    const SPA_PGS: &[(u32, &str)] = &[(1, "Subtitles"), (3, "spa"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];
    const ENG_VOBSUB: &[(u32, &str)] = &[(1, "Subtitles"), (3, "eng"), (5, "S_VOBSUB"), (7, "Dvd Subtitles")];

    /// The film's UHD Blu-ray: titles 0 and 1 two cuts (UHD video, English DTS-HD MA 5.1, English PGS), title 2 a
    /// Blu-ray title with Spanish tracks beside the English ones, title 3 one whose only audio is Spanish, title 4 one
    /// without a duration (attribute 9).
    fn film_disc() -> Disc {
        let cut: &[(u32, &[(u32, &str)])] = &[(0, UHD_VIDEO), (1, ENG_DTS_HD_MA_6CH), (2, ENG_PGS)];
        let mixed: &[(u32, &[(u32, &str)])] =
            &[(0, HD_VIDEO), (1, ENG_AC3_6CH), (2, SPA_AC3_2CH), (3, ENG_PGS), (4, SPA_PGS)];
        let spanish: &[(u32, &[(u32, &str)])] = &[(0, HD_VIDEO), (1, SPA_AC3_2CH), (2, ENG_PGS)];
        let t0: &[(u32, &str)] =
            &[(2, "Film"), (8, "24"), (9, "1:58:42"), (11, "40000000000"), (16, "00800.mpls"), (26, "1,2,3")];
        let t1: &[(u32, &str)] =
            &[(2, "Film"), (8, "26"), (9, "2:06:10"), (11, "42000000000"), (16, "00801.mpls"), (26, "1,2,4")];
        let t2: &[(u32, &str)] = &[(8, "20"), (9, "1:40:00"), (11, "30000000000"), (16, "00802.mpls"), (26, "5")];
        let t3: &[(u32, &str)] = &[(8, "18"), (9, "1:35:00"), (11, "28000000000"), (16, "00803.mpls"), (26, "6")];
        let t4: &[(u32, &str)] = &[(8, "1"), (11, "1000000"), (16, "00804.mpls"), (26, "7")];
        let text = scan_text(
            &[(1, "Blu-ray disc"), (2, "Film"), (32, "FILM")],
            &[(0, t0, cut), (1, t1, cut), (2, t2, mixed), (3, t3, spanish), (4, t4, cut)],
        );
        disc(&parse(&text)).unwrap()
    }

    /// A Blu-ray of three episode titles (HD video, English Dolby Digital 5.1, English PGS).
    fn office_disc() -> Disc {
        let streams: &[(u32, &[(u32, &str)])] = &[(0, HD_VIDEO), (1, ENG_AC3_6CH), (2, ENG_PGS)];
        let t0: &[(u32, &str)] = &[(8, "6"), (9, "0:21:37"), (11, "4800000000"), (16, "00001.mpls"), (26, "1")];
        let t1: &[(u32, &str)] = &[(8, "6"), (9, "0:21:30"), (11, "4700000000"), (16, "00002.mpls"), (26, "2")];
        let t3: &[(u32, &str)] = &[(8, "12"), (9, "0:43:05"), (11, "9500000000"), (16, "00004.mpls"), (26, "4")];
        let text = scan_text(
            &[(1, "Blu-ray disc"), (2, "EXAMPLE SHOW S3D3"), (32, "EXAMPLE SHOW S3D3")],
            &[(0, t0, streams), (1, t1, streams), (3, t3, streams)],
        );
        disc(&parse(&text)).unwrap()
    }

    /// A DVD of two titles that share the segment map "1" and the duration 1:38:12 (Review Focus 1): no attribute 16,
    /// attribute 24 "01" and "02", so only attribute 24 tells them apart.
    fn dvd_disc() -> Disc {
        let streams: &[(u32, &[(u32, &str)])] = &[(0, DVD_VIDEO), (1, ENG_AC3_6CH), (2, ENG_VOBSUB)];
        let t0: &[(u32, &str)] = &[(8, "20"), (9, "1:38:12"), (11, "6000000000"), (24, "01"), (26, "1")];
        let t1: &[(u32, &str)] = &[(8, "2"), (9, "1:38:12"), (11, "700000000"), (24, "02"), (26, "1")];
        let text = scan_text(&[(1, "DVD disc"), (32, "DVD_VIDEO")], &[(0, t0, streams), (1, t1, streams)]);
        disc(&parse(&text)).unwrap()
    }

    /// A `/movie/{id}` record as TMDb documents it, already parsed.
    fn film_record(id: i64, title: &str, year: i32, imdb_id: Option<&str>) -> TmdbRecord {
        TmdbRecord {
            identity: TmdbIdentity { kind: TmdbKind::Movie, id },
            title: title.to_string(),
            original_title: title.to_string(),
            year: Some(year),
            original_language: Some("en".to_string()),
            runtime: Some(119),
            imdb_id: imdb_id.map(str::to_string),
            tvdb_id: None,
            seasons: Vec::new(),
        }
    }

    /// A `/tv/{id}` record with season 3's episodes 15 and 16 merged from `/tv/{id}/season/3`.
    fn office_record() -> TmdbRecord {
        TmdbRecord {
            identity: OFFICE,
            title: "The Office".to_string(),
            original_title: "The Office".to_string(),
            year: Some(2005),
            original_language: Some("en".to_string()),
            runtime: None,
            imdb_id: Some("tt0002316".to_string()),
            tvdb_id: Some(102316),
            seasons: vec![TmdbSeason {
                season_number: 3,
                episode_count: Some(25),
                episodes: vec![
                    TmdbEpisode { episode_number: 15, name: Some("Example Fifteen".to_string()), runtime: Some(22) },
                    TmdbEpisode { episode_number: 16, name: Some("Example Sixteen".to_string()), runtime: Some(22) },
                ],
            }],
        }
    }

    /// A planned feature as the planner flags a theatrical or main cut: theatrical without an edition, or when the
    /// edition names the theatrical cut.
    fn feature_role(edition: Option<&str>) -> PlannedRole {
        PlannedRole::Feature { edition: edition.map(str::to_string), theatrical: edition.is_none() || is_theatrical(edition) }
    }

    /// A planned feature the planner did not flag theatrical and left without an edition: an unnamed cut (8.3 rule 2's
    /// Other suffix) or a structural rival.
    fn unnamed_role() -> PlannedRole {
        PlannedRole::Feature { edition: None, theatrical: false }
    }

    /// The owner's Resolve collision answering `path` with `action`.
    fn answered(path: &str, action: Collision) -> Overrides {
        Overrides { collision: vec![CollisionAnswer { path: path.to_string(), action }], ..Overrides::default() }
    }

    /// Title 0's plain final path under `settings()` with the film's record.
    const FILM_PATH: &str = r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv";

    fn episode_role(season: Option<u32>, episodes: &[u32], name: Option<&str>) -> PlannedRole {
        PlannedRole::Episode(Episode { season, episodes: episodes.to_vec(), name: name.map(str::to_string) })
    }

    /// A plan as 6.1 step 5 leaves it: TheDiscDB knew the disc (identity 0.95, title 0.98, Direct), an English work,
    /// the given titles in the given order with their keys (spec 5), no review reason and no note.
    fn plan_of(disc: &Disc, kind: Kind, tmdb: Option<TmdbIdentity>, roles: &[(u32, PlannedRole)]) -> Plan {
        let titles = roles
            .iter()
            .enumerate()
            .map(|(order, (number, role))| {
                let title = disc.title(*number).unwrap();
                PlannedTitle {
                    key: title_key(order, title, disc.is_dvd),
                    title: *number,
                    role: role.clone(),
                    seconds: title.seconds,
                    size: title.size,
                    source_file: title.source_file.clone(),
                    suggested: false,
                    remembered: false,
                }
            })
            .collect();
        Plan {
            identity: Identity {
                step: IdentityStep::DiscdbOne,
                confidence: 0.95,
                tmdb,
                title: None,
                year: None,
                original_language: Some("en".to_string()),
                review: None,
                plausible: Vec::new(),
                runtime_check: None,
                lookup_pending: false,
                evidence: Vec::new(),
            },
            kind,
            kind_source: KindSource::Discdb,
            titles,
            mode: Mode::Direct,
            identity_confidence: 0.95,
            title_confidence: 0.98,
            reviews: Vec::new(),
            notes: Vec::new(),
            wanted: wanted_languages(Some("en")),
            session: None,
            evidence: PlanEvidence {
                name: None,
                candidates: roles.iter().map(|(number, _)| *number).collect(),
                groups: Vec::new(),
                rivals: Vec::new(),
                title_rule: match kind {
                    Kind::Movie => Some(TitleRule::Database),
                    Kind::Tv => None,
                },
                labels: Vec::new(),
                discdb_matches: Vec::new(),
                runtime: RuntimeFacts::default(),
                sentences: Vec::new(),
            },
        }
    }

    /// Spec 14's defaults with both library roots set.
    fn settings() -> Settings {
        Settings { movies_root: Some(MOVIES.to_string()), tv_root: Some(SHOWS.to_string()), ..Settings::default() }
    }

    /// The inputs of the planned title `number`: no measured facts, no rip review, no read error, no override.
    fn inputs<'a>(
        plan: &'a Plan,
        disc: &'a Disc,
        number: u32,
        record: Option<&'a TmdbRecord>,
        library: &'a LibraryFacts,
        settings: &'a Settings,
    ) -> DecisionInputs<'a> {
        DecisionInputs {
            plan,
            disc,
            planned: plan.titles.iter().find(|planned| planned.title == number).unwrap(),
            title: disc.title(number).unwrap(),
            audio: AudioFacts::default(),
            subtitles: SubtitleFacts::default(),
            record,
            library,
            rip_reviews: Vec::new(),
            read_errors: Vec::new(),
            overrides: None,
            settings,
        }
    }

    fn path_of(decision: &FileDecision) -> &str {
        decision.path.as_ref().expect("the decision has a final path").path.as_str()
    }

    #[test]
    fn a_movie_title_gets_its_tracks_layout_path_and_confidences_and_files_automatically() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.title_key, "t00-00800");
        assert_eq!(decision.audio.kept.iter().map(|track| track.stream).collect::<Vec<u32>>(), vec![1]);
        assert_eq!(decision.audio.confidence, 0.95);
        assert_eq!(
            decision.subtitles.kept.iter().map(|track| (track.stream, track.role)).collect::<Vec<_>>(),
            vec![(2, SubtitleRole::Regular)]
        );
        assert_eq!(decision.subtitles.confidence, 0.90);
        // 8.10: the video track is default, then the default English audio; the default audio is English and no forced
        // track is kept, so no subtitle track is default (`default_subtitle` gives None); chapters are kept.
        assert_eq!(
            decision.layout.tracks.iter().map(|track| (track.stream, track.kind, track.default)).collect::<Vec<_>>(),
            vec![(0, StreamKind::Video, true), (1, StreamKind::Audio, true), (2, StreamKind::Subtitles, false)]
        );
        assert!(decision.layout.chapters_kept);
        assert_eq!(decision.resolution.as_deref(), Some("2160p"));
        assert_eq!(decision.edition, None);
        assert_eq!(
            decision.path,
            Some(NamedPath {
                root: MOVIES.to_string(),
                components: vec![
                    "Film (2010) {tmdb-12345}".to_string(),
                    "Film (2010) {tmdb-12345} - 2160p.mkv".to_string(),
                ],
                path: r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv".to_string(),
            })
        );
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 0.98, track: 0.90 });
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert!(decision.notes.is_empty(), "{:?}", decision.notes);
        assert!(decision.read_errors.is_empty());
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn a_second_cut_is_filed_under_its_edition_name_in_the_combined_style() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[(0, feature_role(Some("Theatrical Cut"))), (1, feature_role(Some("Unrated Cut")))],
        );
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        assert_eq!(settings.edition_style, EditionStyle::Combined);
        let theatrical = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        let unrated = decide(&inputs(&plan, &disc, 1, Some(&record), &library, &settings)).unwrap();
        assert_eq!(theatrical.edition.as_deref(), Some("Theatrical Cut"));
        assert_eq!(
            path_of(&theatrical),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p Theatrical Cut {edition-Theatrical Cut}.mkv"
        );
        assert_eq!(unrated.title_key, "t01-00801");
        assert_eq!(unrated.edition.as_deref(), Some("Unrated Cut"));
        assert_eq!(
            path_of(&unrated),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p Unrated Cut {edition-Unrated Cut}.mkv"
        );
    }

    #[test]
    fn the_plex_edition_style_files_a_cut_in_a_folder_of_its_own() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[(0, feature_role(Some("Theatrical Cut"))), (1, feature_role(Some("Unrated Cut")))],
        );
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = Settings { edition_style: EditionStyle::Plex, ..settings() };
        let unrated = decide(&inputs(&plan, &disc, 1, Some(&record), &library, &settings)).unwrap();
        assert_eq!(
            path_of(&unrated),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345} {edition-Unrated Cut}\Film (2010) {tmdb-12345} {edition-Unrated Cut} - 2160p.mkv"
        );
    }

    #[test]
    fn a_lone_theatrical_cut_is_named_when_the_library_folder_already_holds_an_edition_named_file() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts { folder_has_edition_named_file: true, ..LibraryFacts::default() };
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.edition.as_deref(), Some("Theatrical Cut"));
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p Theatrical Cut {edition-Theatrical Cut}.mkv"
        );
    }

    #[test]
    fn two_structural_rivals_get_no_edition_name_until_the_owner_names_them() {
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        plan.evidence.title_rule = Some(TitleRule::Structure);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.edition, None);
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv"
        );
    }

    #[test]
    fn an_unnamed_cut_keeps_no_edition_name_beside_the_theatrical_cut() {
        // Task 7's plan of two labelled cuts, FPL_MainFeature and FPL_MainFeature_DC (8.3 rule 2): the first is the
        // "Theatrical Cut", the second an unnamed cut (edition None, not theatrical) held with `unnamed_cut`.
        let disc = film_disc();
        let mut plan =
            plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(Some("Theatrical Cut"))), (1, unnamed_role())]);
        plan.evidence.title_rule = Some(TitleRule::Labels);
        plan.title_confidence = 0.60;
        plan.mode = Mode::Copy;
        plan.reviews = vec![ReviewReason::UnnamedCut];
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let theatrical = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        let unnamed = decide(&inputs(&plan, &disc, 1, Some(&record), &library, &settings)).unwrap();
        assert_eq!(theatrical.edition.as_deref(), Some("Theatrical Cut"));
        // Not a second "Theatrical Cut": the owner names it in review (8.4), and the two files never share a path.
        assert_eq!(unnamed.edition, None);
        assert_eq!(
            path_of(&unnamed),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv"
        );
        assert_ne!(path_of(&unnamed), path_of(&theatrical));
        assert_eq!(unnamed.reviews, vec![ReviewReason::UnnamedCut]);
        assert_eq!(unnamed.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn an_edition_name_is_cut_to_32_characters() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[
                (0, feature_role(Some("Theatrical Cut"))),
                (1, feature_role(Some("The Extended Director's Cut With Extra Scenes"))),
            ],
        );
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 1, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.edition.as_deref(), Some("The Extended Director's Cut With"));
    }

    #[test]
    fn the_owners_cut_name_replaces_the_planned_edition_and_sets_the_title_confidence_to_100() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[(0, feature_role(Some("Theatrical Cut"))), (1, feature_role(Some("Unrated Cut")))],
        );
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides {
            titles: vec![TitleOverrideEntry { title: 1, role: TitleRole::Cut { edition: "Director's Cut".to_string() } }],
            ..Overrides::default()
        };
        let mut input = inputs(&plan, &disc, 1, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(decision.edition.as_deref(), Some("Director's Cut"));
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p Director's Cut {edition-Director's Cut}.mkv"
        );
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 1.00, track: 0.90 });
    }

    #[test]
    fn jellyfin10_compat_adds_the_tmdbid_tag_to_the_folder_and_the_file() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = Settings { jellyfin10_compat: true, ..settings() };
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345} [tmdbid-12345]\Film (2010) {tmdb-12345} [tmdbid-12345] - 2160p.mkv"
        );
        assert!(decision.notes.is_empty(), "{:?}", decision.notes);
    }

    #[test]
    fn a_year_like_tmdb_id_without_an_imdb_id_leaves_the_compat_tag_out_with_its_note_and_still_files() {
        let disc = film_disc();
        let year_like = TmdbIdentity { kind: TmdbKind::Movie, id: 1999 };
        let plan = plan_of(&disc, Kind::Movie, Some(year_like), &[(0, feature_role(None))]);
        let record = film_record(1999, "Film", 2010, None);
        let library = LibraryFacts::default();
        let settings = Settings { jellyfin10_compat: true, ..settings() };
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-1999}\Film (2010) {tmdb-1999} - 2160p.mkv"
        );
        assert!(decision.notes.iter().any(|noted| noted.note == Note::CompatTagOmitted), "{:?}", decision.notes);
        assert!(decision.reviews.is_empty());
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn without_a_movies_root_no_path_is_built_and_the_job_is_held_without_a_review_reason() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = Settings::default();
        assert_eq!(settings.movies_root, None);
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.path, None);
        assert_eq!(decision.resolution.as_deref(), Some("2160p"));
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 0.98, track: 0.90 });
        assert!(decision.reviews.is_empty());
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn without_a_tmdb_record_no_path_is_built_and_the_job_is_held() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, None, &library, &settings)).unwrap();
        assert_eq!(decision.path, None);
        assert!(decision.reviews.is_empty());
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn a_tmdb_record_of_another_identity_is_refused() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(54321, "Other Film", 2011, Some("tt0054321"));
        let library = LibraryFacts::default();
        let settings = settings();
        assert_eq!(
            decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)),
            Err("the TMDb record is movie 54321 but the job's identity is movie 12345: title and year come from the \
                 identity's own record (spec 8.9)"
                .to_string())
        );
    }

    #[test]
    fn the_owners_set_identity_names_the_file_and_sets_the_identity_confidence_to_100() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(54321, "Other Film", 2011, Some("tt0054321"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides =
            Overrides { identity: Some(TmdbIdentity { kind: TmdbKind::Movie, id: 54321 }), ..Overrides::default() };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Other Film (2011) {tmdb-54321}\Other Film (2011) {tmdb-54321} - 2160p.mkv"
        );
        assert_eq!(decision.confidences, Confidences { identity: 1.00, title: 0.98, track: 0.90 });
    }

    #[test]
    fn a_file_already_at_the_final_path_raises_path_collision() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts { path_collision: true, ..LibraryFacts::default() };
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.reviews, vec![ReviewReason::PathCollision]);
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv"
        );
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn keep_both_files_the_new_copy_under_the_distinguishing_label_2() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts { path_collision: true, ..LibraryFacts::default() };
        let settings = settings();
        let overrides = answered(FILM_PATH, Collision::KeepBoth);
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p 2.mkv"
        );
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.disposition, Disposition::AutoFile);
        let gone = LibraryFacts::default();
        let mut input = inputs(&plan, &disc, 0, Some(&record), &gone, &settings);
        input.overrides = Some(&overrides);
        let plain = decide(&input).unwrap();
        assert_eq!(
            path_of(&plain),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv"
        );
        assert!(plain.reviews.is_empty(), "{:?}", plain.reviews);
    }

    #[test]
    fn replace_files_at_the_same_path_without_path_collision() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts { path_collision: true, ..LibraryFacts::default() };
        let settings = settings();
        let overrides = answered(FILM_PATH, Collision::Replace);
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv"
        );
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn a_discarded_job_is_never_decided() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts { path_collision: true, ..LibraryFacts::default() };
        let settings = settings();
        let overrides = answered(FILM_PATH, Collision::Discard);
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        assert_eq!(
            decide(&input),
            Err("the owner discarded the job (spec 11 Resolve collision), so title 0 (t00-00800) is not decided".to_string())
        );
    }

    #[test]
    fn a_blank_library_root_builds_no_path_and_holds_the_job() {
        // F14: a settings file with "movies_root": "" (or blanks, or separators only) is a root not set: no relative path
        // is built, and the decision needs review, as with no root at all; the same for tv_root.
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        for blank in ["", "  ", "/", r"\\"] {
            let settings = Settings { movies_root: Some(blank.to_string()), ..settings() };
            let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
            assert_eq!(decision.path, None, "{blank:?}");
            assert_eq!(decision.disposition, Disposition::NeedsReview, "{blank:?}");
        }
        let office = office_disc();
        let episodes = plan_of(&office, Kind::Tv, Some(OFFICE), &[(0, episode_role(Some(3), &[15], Some("Example Fifteen")))]);
        let shows = office_record();
        let settings = Settings { tv_root: Some(String::new()), ..settings() };
        let decision = decide(&inputs(&episodes, &office, 0, Some(&shows), &library, &settings)).unwrap();
        assert_eq!((decision.path, decision.disposition), (None, Disposition::NeedsReview));
    }

    #[test]
    fn a_resolved_collision_covers_only_the_final_path_the_owner_answered() {
        // F5: in one review the owner sets the identity to another film (54321) and answers Replace for the collision
        // shown, the plain path of the planned identity. The decision made again builds the other film's path, where the
        // library holds another file: that collision was never answered, so path_collision holds the job again.
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let other = film_record(54321, "Other Film", 2011, Some("tt0054321"));
        let library = LibraryFacts { path_collision: true, ..LibraryFacts::default() };
        let settings = settings();
        for action in [Collision::Replace, Collision::KeepBoth] {
            let overrides = Overrides {
                identity: Some(TmdbIdentity { kind: TmdbKind::Movie, id: 54321 }),
                ..answered(FILM_PATH, action)
            };
            let mut input = inputs(&plan, &disc, 0, Some(&other), &library, &settings);
            input.overrides = Some(&overrides);
            let decision = decide(&input).unwrap();
            assert_eq!(
                path_of(&decision),
                r"\\nas\media\Movies\Other Film (2011) {tmdb-54321}\Other Film (2011) {tmdb-54321} - 2160p.mkv",
                "{action:?}"
            );
            assert_eq!(decision.reviews, vec![ReviewReason::PathCollision], "{action:?}");
            assert_eq!(decision.disposition, Disposition::NeedsReview, "{action:?}");
        }
        // The answer matches its path whatever the case or the separators (spec 7's Windows shares).
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let overrides = answered(&FILM_PATH.to_lowercase().replace('\\', "/"), Collision::Replace);
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
    }

    #[test]
    fn a_record_fetched_after_planning_gives_the_wanted_languages_its_original_language() {
        // F2: the plan was made with TMDb pending, so its identity (TheDiscDB's 12345) has no original language and its
        // wanted languages are English only. The record fetched before the file phase is the same identity's and says the
        // film is Spanish: the Spanish track is wanted (R4, 8.6 "Languages"), not dropped as an unwanted language.
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(2, feature_role(None))]);
        plan.identity.original_language = None;
        plan.wanted = wanted_languages(None);
        let mut record = film_record(12345, "Film", 2010, Some("tt0012345"));
        record.original_language = Some("es".to_string());
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 2, Some(&record), &library, &settings)).unwrap();
        let mut kept: Vec<u32> = decision.audio.kept.iter().map(|track| track.stream).collect();
        kept.sort_unstable();
        assert_eq!(kept, vec![1, 2]);
        assert!(!decision.audio.dropped.contains(&(2, AudioDrop::UnwantedLanguage)), "{:?}", decision.audio.dropped);
        // Without the record the plan's English-only set stands.
        let unnamed = decide(&inputs(&plan, &disc, 2, None, &library, &settings)).unwrap();
        assert!(unnamed.audio.dropped.contains(&(2, AudioDrop::UnwantedLanguage)), "{:?}", unnamed.audio.dropped);
    }

    #[test]
    fn a_lone_unnamed_cut_keeps_no_edition_and_cannot_be_remembered_until_the_owner_names_it() {
        // F11: the plan's only feature is an unnamed cut (8.3 rule 2's FPL_MainFeature_IMAX: an Other suffix, edition None,
        // not theatrical, 0.60 with unnamed_cut). The film's library folder already holds an edition-named file, which
        // names a theatrical cut "Theatrical Cut" (8.4); the unnamed cut keeps no edition, and Remember refuses it.
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role())]);
        plan.title_confidence = 0.60;
        plan.reviews = vec![ReviewReason::UnnamedCut];
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts { folder_has_edition_named_file: true, ..LibraryFacts::default() };
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.edition, None);
        assert_eq!(path_of(&decision), FILM_PATH);
        assert_eq!(
            remember(FINGERPRINT, &disc, &plan, &[decision], &Overrides::default()),
            Err("planned title 0 (t00-00800) is a cut without an edition name to remember: name it first (spec 8.4)"
                .to_string())
        );
        // The theatrical cut alone, beside the same edition-named file, is named "Theatrical Cut" and remembered.
        let theatrical = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let decision = decide(&inputs(&theatrical, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.edition.as_deref(), Some("Theatrical Cut"));
        let remembered = remember(FINGERPRINT, &disc, &theatrical, &[decision], &Overrides::default()).unwrap();
        assert_eq!(remembered.titles[0].role, TitleRole::Feature);
    }

    #[test]
    fn read_errors_are_carried_with_their_offsets_and_raise_read_errors() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let errors = vec![ReadError { offset: "1048576".to_string(), count: 3, message: SCRAMBLED.to_string() }];
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.rip_reviews = vec![ReviewReason::ReadErrors];
        input.read_errors = errors.clone();
        let decision = decide(&input).unwrap();
        assert_eq!(decision.reviews, vec![ReviewReason::ReadErrors]);
        assert_eq!(decision.read_errors, errors);
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 0.98, track: 0.90 });
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn read_errors_is_raised_exactly_when_the_rip_logged_read_errors() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let mut carried_without_errors = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        carried_without_errors.rip_reviews = vec![ReviewReason::ReadErrors];
        let decision = decide(&carried_without_errors).unwrap();
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.disposition, Disposition::AutoFile);
        let mut errors_without_the_reason = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        errors_without_the_reason.read_errors =
            vec![ReadError { offset: "1048576".to_string(), count: 1, message: SCRAMBLED.to_string() }];
        assert_eq!(decide(&errors_without_the_reason).unwrap().reviews, vec![ReviewReason::ReadErrors]);
    }

    #[test]
    fn verify_failed_and_copy_hash_errors_from_the_rip_hold_the_job_and_lower_no_confidence() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.rip_reviews = vec![ReviewReason::VerifyFailed, ReviewReason::CopyHashErrors];
        let decision = decide(&input).unwrap();
        assert_eq!(decision.reviews, vec![ReviewReason::CopyHashErrors, ReviewReason::VerifyFailed]);
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 0.98, track: 0.90 });
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn the_plans_review_reasons_and_notes_are_carried_into_the_decision() {
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        plan.reviews = vec![ReviewReason::IdentityAmbiguous];
        plan.notes = vec![Noted {
            note: Note::LabelRuleUnavailable,
            stream: None,
            detail: "MakeMKV did not report a Java runtime in use".to_string(),
        }];
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.reviews, vec![ReviewReason::IdentityAmbiguous]);
        assert_eq!(decision.notes, plan.notes);
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn the_owners_assign_titles_answers_the_plans_title_review_reasons() {
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        plan.reviews = vec![ReviewReason::TitleAmbiguous, ReviewReason::MultipleFeatures, ReviewReason::UnnamedCut];
        plan.title_confidence = 0.60;
        plan.mode = Mode::Copy;
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides =
            Overrides { titles: vec![TitleOverrideEntry { title: 0, role: TitleRole::Feature }], ..Overrides::default() };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 1.00, track: 0.90 });
        assert_eq!(decision.disposition, Disposition::AutoFile);
        let mut also_identity = plan.clone();
        also_identity.reviews.push(ReviewReason::IdentityAmbiguous);
        let mut input = inputs(&also_identity, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        assert_eq!(decide(&input).unwrap().reviews, vec![ReviewReason::IdentityAmbiguous]);
    }

    #[test]
    fn the_owners_set_identity_answers_the_plans_identity_review_reasons() {
        let disc = film_disc();
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides { identity: Some(FILM), ..Overrides::default() };
        let mut ambiguous = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        ambiguous.reviews = vec![ReviewReason::IdentityAmbiguous];
        ambiguous.identity_confidence = 0.50;
        let mut unknown = plan_of(&disc, Kind::Movie, None, &[(0, feature_role(None))]);
        unknown.reviews = vec![ReviewReason::IdentityUnknown];
        unknown.identity_confidence = 0.0;
        for plan in [&ambiguous, &unknown] {
            let mut input = inputs(plan, &disc, 0, Some(&record), &library, &settings);
            input.overrides = Some(&overrides);
            let decision = decide(&input).unwrap();
            assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
            assert_eq!(decision.confidences, Confidences { identity: 1.00, title: 0.98, track: 0.90 });
            assert_eq!(
                path_of(&decision),
                r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv"
            );
            assert_eq!(decision.disposition, Disposition::AutoFile);
        }
    }

    #[test]
    fn no_wanted_audio_from_the_audio_rule_holds_the_job() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(3, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 3, Some(&record), &library, &settings)).unwrap();
        assert!(decision.audio.kept.is_empty());
        assert_eq!(decision.reviews, vec![ReviewReason::NoWantedAudio]);
        assert_eq!(decision.resolution.as_deref(), Some("1080p"));
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn a_plan_title_confidence_of_085_files_automatically() {
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        plan.title_confidence = 0.85;
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.confidences.title, 0.85);
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn a_plan_title_confidence_of_060_holds_the_job_without_a_review_reason_of_its_own() {
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        plan.title_confidence = 0.60;
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.confidences.title, 0.60);
        assert!(decision.reviews.is_empty());
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn an_auto_threshold_of_095_holds_the_090_track_until_the_owner_overrides_every_track() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = Settings { auto_threshold: 0.95, ..settings() };
        let held = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(held.confidences.track, 0.90);
        assert_eq!(held.disposition, Disposition::NeedsReview);
        let overrides = Overrides {
            audio: vec![AudioTrackOverride {
                title: 0,
                stream: 1,
                action: AudioOverride::Keep { order: 0, default: true },
            }],
            subtitles: vec![SubtitleTrackOverride {
                title: 0,
                stream: 2,
                action: SubtitleOverride::Keep { role: SubtitleRole::Regular },
            }],
            ..Overrides::default()
        };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let approved = decide(&input).unwrap();
        assert_eq!(approved.confidences, Confidences { identity: 0.95, title: 0.98, track: 1.00 });
        assert_eq!(approved.disposition, Disposition::AutoFile);
    }

    #[test]
    fn the_owners_audio_overrides_set_audio_to_100_and_the_track_stays_at_the_subtitles_090() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides {
            audio: vec![AudioTrackOverride {
                title: 0,
                stream: 1,
                action: AudioOverride::Keep { order: 0, default: true },
            }],
            ..Overrides::default()
        };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(decision.audio.confidence, 1.00);
        assert_eq!(decision.subtitles.confidence, 0.90);
        assert_eq!(decision.confidences.track, 0.90);
    }

    #[test]
    fn the_owners_audio_and_subtitle_overrides_together_set_the_track_confidence_to_100() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides {
            audio: vec![AudioTrackOverride {
                title: 0,
                stream: 1,
                action: AudioOverride::Keep { order: 0, default: true },
            }],
            subtitles: vec![SubtitleTrackOverride {
                title: 0,
                stream: 2,
                action: SubtitleOverride::Keep { role: SubtitleRole::Regular },
            }],
            ..Overrides::default()
        };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(decision.subtitles.confidence, 1.00);
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 0.98, track: 1.00 });
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn a_title_the_owner_ignored_is_refused() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides =
            Overrides { titles: vec![TitleOverrideEntry { title: 0, role: TitleRole::Ignored }], ..Overrides::default() };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        assert_eq!(
            decide(&input),
            Err("the owner's Assign titles ignores title 0 (t00-00800), so nothing is filed for it".to_string())
        );
    }

    #[test]
    fn a_planned_title_that_is_not_in_the_plan_is_refused() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let stranger = PlannedTitle { key: "t05-00800".to_string(), ..plan.titles[0].clone() };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.planned = &stranger;
        assert_eq!(decide(&input), Err("the plan holds no title 0 with key t05-00800".to_string()));
    }

    #[test]
    fn a_planned_title_decided_from_another_scan_title_is_refused() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.title = disc.title(1).unwrap();
        assert_eq!(
            decide(&input),
            Err("planned title 0 (t00-00800) cannot be decided from scan title 1".to_string())
        );
    }

    #[test]
    fn a_mapped_episode_is_filed_in_its_season_folder_under_its_title() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(0, episode_role(Some(3), &[15], Some("Example Fifteen")))]);
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.title_key, "t00-00001");
        assert_eq!(decision.resolution.as_deref(), Some("1080p"));
        assert_eq!(decision.edition, None);
        assert_eq!(
            decision.path,
            Some(NamedPath {
                root: SHOWS.to_string(),
                components: vec![
                    "The Office (2005) {tmdb-2316}".to_string(),
                    "Season 03".to_string(),
                    "The Office (2005) - S03E15 - Example Fifteen.mkv".to_string(),
                ],
                path: r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E15 - Example Fifteen.mkv"
                    .to_string(),
            })
        );
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn a_multi_episode_title_is_filed_under_its_episode_range_without_the_slash_of_its_name() {
        let disc = office_disc();
        let plan = plan_of(
            &disc,
            Kind::Tv,
            Some(OFFICE),
            &[(3, episode_role(Some(3), &[12, 13], Some("Example One/Example Two")))],
        );
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 3, Some(&record), &library, &settings)).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E12-E13 - Example OneExample Two.mkv"
        );
    }

    #[test]
    fn an_episode_without_a_name_of_its_own_takes_tmdbs_episode_name() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(1, episode_role(Some(3), &[16], None))]);
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 1, Some(&record), &library, &settings)).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E16 - Example Sixteen.mkv"
        );
    }

    #[test]
    fn an_episode_without_a_season_builds_no_path_and_raises_tv_unmapped() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(0, episode_role(None, &[15], Some("Example Fifteen")))]);
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.path, None);
        assert_eq!(decision.reviews, vec![ReviewReason::TvUnmapped]);
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn the_owners_season_fills_a_planned_episode_without_one() {
        let disc = office_disc();
        let mut plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(0, episode_role(None, &[15], Some("Example Fifteen")))]);
        plan.reviews = vec![ReviewReason::TvUnmapped];
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides { season: Some(3), ..Overrides::default() };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E15 - Example Fifteen.mkv"
        );
        assert_eq!(decision.reviews, vec![ReviewReason::TvUnmapped]);
        assert_eq!(decision.confidences.title, 0.98);
    }

    #[test]
    fn the_owners_episode_assignment_names_the_file_and_sets_the_title_confidence_to_100() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(1, episode_role(None, &[1], None))]);
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides {
            titles: vec![TitleOverrideEntry { title: 1, role: TitleRole::Episodes { season: 3, episodes: vec![16] } }],
            ..Overrides::default()
        };
        let mut input = inputs(&plan, &disc, 1, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E16 - Example Sixteen.mkv"
        );
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 1.00, track: 0.90 });
        assert!(decision.reviews.is_empty());
    }

    #[test]
    fn without_a_tv_root_no_episode_path_is_built() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(0, episode_role(Some(3), &[15], Some("Example Fifteen")))]);
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = Settings { movies_root: Some(MOVIES.to_string()), ..Settings::default() };
        assert_eq!(settings.tv_root, None);
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.path, None);
        assert!(decision.reviews.is_empty());
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn a_file_decision_writes_its_fields_in_order_and_round_trips_through_json() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        let value = serde_json::to_value(&decision).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "title_key",
                "audio",
                "subtitles",
                "layout",
                "resolution",
                "edition",
                "path",
                "confidences",
                "reviews",
                "notes",
                "read_errors",
                "disposition",
            ]
        );
        assert_eq!(value["edition"], Value::Null);
        assert_eq!(value["confidences"], json!({"identity": 0.95, "title": 0.98, "track": 0.90}));
        assert_eq!(value["disposition"], json!("auto_file"));
        let back: FileDecision = serde_json::from_value(value).unwrap();
        assert_eq!(back, decision);
    }

    #[test]
    fn the_owners_set_identity_brings_the_original_language_into_the_audio_rule() {
        // Planned with identity_unknown (English only), then given a Spanish film's identity at the disposition gate.
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, None, &[(2, feature_role(None))]);
        plan.identity.step = IdentityStep::Nothing;
        plan.identity.original_language = None;
        plan.identity_confidence = 0.0;
        plan.reviews = vec![ReviewReason::IdentityUnknown];
        plan.wanted = wanted_languages(None);
        let spanish_film = TmdbIdentity { kind: TmdbKind::Movie, id: 54321 };
        let record = TmdbRecord {
            original_language: Some("es".to_string()),
            ..film_record(54321, "El Film", 2010, Some("tt0054321"))
        };
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides { identity: Some(spanish_film), ..Overrides::default() };
        let mut input = inputs(&plan, &disc, 2, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        // Title 2's Spanish Dolby Digital stereo (stream 2) is the original language: kept, first, default, original.
        assert_eq!(decision.audio.kept.iter().map(|track| track.stream).collect::<Vec<u32>>(), vec![2, 1]);
        let first = &decision.audio.kept[0];
        assert_eq!((first.stream, first.default, first.original), (2, true, true));
        assert!(!decision.audio.dropped.iter().any(|(stream, _)| *stream == 2), "{:?}", decision.audio.dropped);
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\El Film (2010) {tmdb-54321}\El Film (2010) {tmdb-54321} - 1080p.mkv"
        );
    }

    #[test]
    fn a_planned_title_the_owners_assign_titles_leaves_out_is_not_filed() {
        // Two structural rivals; the owner assigns only title 1, as a named cut. Assign titles wins whole, as in a re-plan.
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        plan.evidence.title_rule = Some(TitleRule::Structure);
        plan.title_confidence = 0.60;
        plan.mode = Mode::Copy;
        plan.reviews = vec![ReviewReason::MultipleFeatures];
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides {
            titles: vec![TitleOverrideEntry { title: 1, role: TitleRole::Cut { edition: "Director's Cut".to_string() } }],
            ..Overrides::default()
        };
        let mut left_out = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        left_out.overrides = Some(&overrides);
        assert_eq!(
            decide(&left_out),
            Err("the owner's Assign titles does not name title 0 (t00-00800), so nothing is filed for it".to_string())
        );
        let mut named = inputs(&plan, &disc, 1, Some(&record), &library, &settings);
        named.overrides = Some(&overrides);
        let decision = decide(&named).unwrap();
        assert_eq!(decision.edition.as_deref(), Some("Director's Cut"));
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p Director's Cut {edition-Director's Cut}.mkv"
        );
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn an_assigned_title_the_scan_lacks_is_left_out_of_the_editions() {
        // Task 7's `ignored_and_unknown_assigned_titles_are_left_out`, decided: the owner ignores title 0, assigns title 1
        // as the feature and names title 9, which the scan lacks. The re-plan leaves title 9 out and plans title 1 alone
        // with no edition; the decision files the same (Open point 32), never title 1 as a "Theatrical Cut" beside title 9.
        let disc = film_disc();
        assert!(disc.title(9).is_err());
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        plan.evidence.title_rule = Some(TitleRule::Structure);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides {
            titles: vec![
                TitleOverrideEntry { title: 0, role: TitleRole::Ignored },
                TitleOverrideEntry { title: 1, role: TitleRole::Feature },
                TitleOverrideEntry { title: 9, role: TitleRole::Feature },
            ],
            ..Overrides::default()
        };
        let mut input = inputs(&plan, &disc, 1, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(decision.edition, None);
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p.mkv"
        );
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 1.00, track: 0.90 });
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn a_role_whose_kind_differs_from_the_records_builds_no_path() {
        let library = LibraryFacts::default();
        let settings = settings();
        // Episodes on a film's identity: a movie's TMDb ID never names a show folder.
        let film = film_disc();
        let movie_plan = plan_of(&film, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let film_rec = film_record(12345, "Film", 2010, Some("tt0012345"));
        let episodes = Overrides {
            titles: vec![TitleOverrideEntry { title: 0, role: TitleRole::Episodes { season: 1, episodes: vec![1] } }],
            ..Overrides::default()
        };
        let mut input = inputs(&movie_plan, &film, 0, Some(&film_rec), &library, &settings);
        input.overrides = Some(&episodes);
        let decision = decide(&input).unwrap();
        assert_eq!(decision.path, None);
        assert_eq!(decision.disposition, Disposition::NeedsReview);
        // A feature on a show's identity: a show's TMDb ID never names a movie folder.
        let office = office_disc();
        let tv_plan = plan_of(&office, Kind::Tv, Some(OFFICE), &[(0, episode_role(Some(3), &[15], Some("Example Fifteen")))]);
        let show = office_record();
        let feature =
            Overrides { titles: vec![TitleOverrideEntry { title: 0, role: TitleRole::Feature }], ..Overrides::default() };
        let mut input = inputs(&tv_plan, &office, 0, Some(&show), &library, &settings);
        input.overrides = Some(&feature);
        let decision = decide(&input).unwrap();
        assert_eq!(decision.path, None);
        assert_eq!(decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn the_owners_episode_assignment_on_a_movie_plan_files_under_tv_root_with_a_show_identity() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let overrides = Overrides {
            identity: Some(OFFICE),
            titles: vec![TitleOverrideEntry { title: 0, role: TitleRole::Episodes { season: 3, episodes: vec![15] } }],
            ..Overrides::default()
        };
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(decision.edition, None);
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E15 - Example Fifteen.mkv"
        );
        assert_eq!(decision.confidences, Confidences { identity: 1.00, title: 1.00, track: 0.90 });
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    #[test]
    fn an_earlier_filed_copy_name_is_used_at_decide_time() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts {
            earlier_edition_names: BTreeMap::from([("theatrical".to_string(), "Original".to_string())]),
            ..LibraryFacts::default()
        };
        let settings = settings();
        let decision = decide(&inputs(&plan, &disc, 0, Some(&record), &library, &settings)).unwrap();
        assert_eq!(decision.edition.as_deref(), Some("Original"));
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p Original {edition-Original}.mkv"
        );
    }

    #[test]
    fn the_jellyfin_edition_style_writes_the_edition_after_the_resolution() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[(0, feature_role(Some("Theatrical Cut"))), (1, feature_role(Some("Unrated Cut")))],
        );
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let library = LibraryFacts::default();
        let settings = Settings { edition_style: EditionStyle::Jellyfin, ..settings() };
        let unrated = decide(&inputs(&plan, &disc, 1, Some(&record), &library, &settings)).unwrap();
        assert_eq!(
            path_of(&unrated),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 2160p Unrated Cut.mkv"
        );
    }

    #[test]
    fn keep_both_on_an_episode_puts_the_copy_label_after_the_episode_code() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(0, episode_role(Some(3), &[15], Some("Example Fifteen")))]);
        let record = office_record();
        let library = LibraryFacts { path_collision: true, ..LibraryFacts::default() };
        let settings = settings();
        let plain = r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E15 - Example Fifteen.mkv";
        let overrides = answered(plain, Collision::KeepBoth);
        let mut input = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        input.overrides = Some(&overrides);
        let decision = decide(&input).unwrap();
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005) - S03E15 2 - Example Fifteen.mkv"
        );
        assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
        assert_eq!(decision.disposition, Disposition::AutoFile);
    }

    const FINGERPRINT: &str = "5b0f8a3c9d2e4f6071829a3b4c5d6e7f";

    fn sig(kind: StreamKind, language: &str, codec: Family, channels: Option<u32>) -> StreamSignature {
        StreamSignature { kind, language: Some(language.to_string()), codec, channels, position: 0 }
    }

    /// Title `number` of the plan decided with no measured facts, under `overrides` when given.
    fn decided(
        plan: &Plan,
        disc: &Disc,
        number: u32,
        record: &TmdbRecord,
        overrides: Option<&Overrides>,
    ) -> FileDecision {
        let library = LibraryFacts::default();
        let settings = settings();
        let mut input = inputs(plan, disc, number, Some(record), &library, &settings);
        input.overrides = overrides;
        decide(&input).unwrap()
    }

    #[test]
    fn remember_stores_the_identity_every_planned_title_and_every_track_of_the_job() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decision = decided(&plan, &disc, 0, &record, None);
        let remembered = remember(FINGERPRINT, &disc, &plan, &[decision], &Overrides::default()).unwrap();
        assert_eq!(
            remembered,
            Remembered {
                fingerprint: FINGERPRINT.to_string(),
                tmdb: FILM,
                titles: vec![RememberedTitle {
                    source_file: Some("00800.mpls".to_string()),
                    dvd_title: None,
                    segment_map: "1,2,3".to_string(),
                    seconds: 7122,
                    role: TitleRole::Feature,
                    audio: vec![AudioOverrideEntry {
                        signature: sig(StreamKind::Audio, "eng", Family::DtsHdMa, Some(6)),
                        action: AudioOverride::Keep { order: 0, default: true },
                    }],
                    subtitles: vec![SubtitleOverrideEntry {
                        signature: sig(StreamKind::Subtitles, "eng", Family::Pgs, None),
                        action: SubtitleOverride::Keep { role: SubtitleRole::Regular },
                    }],
                }],
            }
        );
    }

    #[test]
    fn a_remembered_file_round_trips_and_matches_its_title_and_streams_back() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decision = decided(&plan, &disc, 0, &record, None);
        let remembered = remember(FINGERPRINT, &disc, &plan, &[decision], &Overrides::default()).unwrap();
        let text = serde_json::to_string(&remembered).unwrap();
        let back: Remembered = serde_json::from_str(&text).unwrap();
        assert_eq!(back, remembered);
        assert_eq!(apply_titles(&back, &disc), BTreeMap::from([(0, &TitleRole::Feature)]));
        let title = disc.title(0).unwrap();
        let matched = matched_titles(&back, &disc);
        assert_eq!(
            apply_audio(&matched[&0].audio, &title.streams),
            BTreeMap::from([(1, &AudioOverride::Keep { order: 0, default: true })])
        );
        assert_eq!(
            apply_subtitles(&matched[&0].subtitles, &title.streams),
            BTreeMap::from([(2, &SubtitleOverride::Keep { role: SubtitleRole::Regular })])
        );
    }

    #[test]
    fn a_remembered_decision_applied_to_the_next_job_sets_the_track_confidence_to_100() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decision = decided(&plan, &disc, 0, &record, None);
        let remembered = remember(FINGERPRINT, &disc, &plan, &[decision], &Overrides::default()).unwrap();
        let library = LibraryFacts::default();
        let settings = settings();
        let matched = matched_titles(&remembered, &disc);
        let mut next = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        next.audio.remembered = Some(matched[&0].audio.as_slice());
        next.subtitles.remembered = Some(matched[&0].subtitles.as_slice());
        let again = decide(&next).unwrap();
        assert_eq!(again.audio.confidence, 1.00);
        assert_eq!(again.subtitles.confidence, 1.00);
        assert_eq!(again.confidences, Confidences { identity: 0.95, title: 0.98, track: 1.00 });
        assert_eq!(again.audio.kept.iter().map(|track| track.stream).collect::<Vec<u32>>(), vec![1]);
    }

    #[test]
    fn a_dropped_track_is_remembered_as_a_drop() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(2, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decision = decided(&plan, &disc, 2, &record, None);
        let remembered = remember(FINGERPRINT, &disc, &plan, &[decision], &Overrides::default()).unwrap();
        assert_eq!(
            remembered.titles[0].audio,
            vec![
                AudioOverrideEntry {
                    signature: sig(StreamKind::Audio, "eng", Family::Ac3, Some(6)),
                    action: AudioOverride::Keep { order: 0, default: true },
                },
                AudioOverrideEntry {
                    signature: sig(StreamKind::Audio, "spa", Family::Ac3, Some(2)),
                    action: AudioOverride::Drop,
                },
            ]
        );
        assert_eq!(
            remembered.titles[0].subtitles,
            vec![
                SubtitleOverrideEntry {
                    signature: sig(StreamKind::Subtitles, "eng", Family::Pgs, None),
                    action: SubtitleOverride::Keep { role: SubtitleRole::Regular },
                },
                SubtitleOverrideEntry {
                    signature: sig(StreamKind::Subtitles, "spa", Family::Pgs, None),
                    action: SubtitleOverride::Drop,
                },
            ]
        );
    }

    #[test]
    fn a_track_the_owner_marked_as_commentary_is_remembered_as_commentary() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(2, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        // Title 2's English Dolby Digital 5.1 (stream 1), which 8.6 keeps: only the owner's mark drops it.
        let overrides = Overrides {
            audio: vec![AudioTrackOverride { title: 2, stream: 1, action: AudioOverride::Commentary }],
            ..Overrides::default()
        };
        let decision = decided(&plan, &disc, 2, &record, Some(&overrides));
        assert!(decision.audio.dropped.contains(&(1, AudioDrop::Reviewed)), "{:?}", decision.audio.dropped);
        let remembered = remember(FINGERPRINT, &disc, &plan, &[decision], &overrides).unwrap();
        assert!(
            remembered.titles[0].audio.contains(&AudioOverrideEntry {
                signature: sig(StreamKind::Audio, "eng", Family::Ac3, Some(6)),
                action: AudioOverride::Commentary,
            }),
            "{:?}",
            remembered.titles[0].audio
        );
    }

    #[test]
    fn on_a_dvd_a_remembered_title_is_matched_back_by_its_dvd_title_number() {
        let disc = dvd_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decision = decided(&plan, &disc, 0, &record, None);
        assert_eq!(decision.title_key, "t00-title01");
        assert_eq!(decision.resolution.as_deref(), Some("480p"));
        assert_eq!(
            path_of(&decision),
            r"\\nas\media\Movies\Film (2010) {tmdb-12345}\Film (2010) {tmdb-12345} - 480p.mkv"
        );
        let remembered = remember(FINGERPRINT, &disc, &plan, &[decision], &Overrides::default()).unwrap();
        assert_eq!(
            remembered.titles,
            vec![RememberedTitle {
                source_file: None,
                dvd_title: Some("01".to_string()),
                segment_map: "1".to_string(),
                seconds: 5892,
                role: TitleRole::Feature,
                audio: remembered.titles[0].audio.clone(),
                subtitles: remembered.titles[0].subtitles.clone(),
            }]
        );
        assert_eq!(remembered.titles[0].audio.len(), 1);
        assert_eq!(apply_titles(&remembered, &disc), BTreeMap::from([(0, &TitleRole::Feature)]));
        assert!(!apply_titles(&remembered, &disc).contains_key(&1));
    }

    #[test]
    fn the_owners_assign_titles_is_remembered_whole_in_title_order() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[(0, feature_role(Some("Theatrical Cut"))), (1, feature_role(Some("Unrated Cut")))],
        );
        let overrides = Overrides {
            titles: vec![
                TitleOverrideEntry { title: 1, role: TitleRole::Cut { edition: "Director's Cut".to_string() } },
                TitleOverrideEntry { title: 3, role: TitleRole::Ignored },
            ],
            ..Overrides::default()
        };
        let remembered = remember(FINGERPRINT, &disc, &plan, &[], &overrides).unwrap();
        // Assign titles wins whole (choice 9): planned title 0, which it does not name, is not remembered.
        assert_eq!(
            remembered.titles,
            vec![
                RememberedTitle {
                    source_file: Some("00801.mpls".to_string()),
                    dvd_title: None,
                    segment_map: "1,2,4".to_string(),
                    seconds: 7570,
                    role: TitleRole::Cut { edition: "Director's Cut".to_string() },
                    audio: vec![],
                    subtitles: vec![],
                },
                RememberedTitle {
                    source_file: Some("00803.mpls".to_string()),
                    dvd_title: None,
                    segment_map: "6".to_string(),
                    seconds: 5700,
                    role: TitleRole::Ignored,
                    audio: vec![],
                    subtitles: vec![],
                },
            ]
        );
    }

    #[test]
    fn remember_leaves_out_an_assigned_title_the_scan_lacks() {
        // The overrides of Task 7's `ignored_and_unknown_assigned_titles_are_left_out`: title 9 is not in the scan, so it
        // is left out as the decision and the re-plan leave it out (choice 9), and the call succeeds.
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        let overrides = Overrides {
            titles: vec![
                TitleOverrideEntry { title: 0, role: TitleRole::Ignored },
                TitleOverrideEntry { title: 1, role: TitleRole::Feature },
                TitleOverrideEntry { title: 9, role: TitleRole::Feature },
            ],
            ..Overrides::default()
        };
        let remembered = remember(FINGERPRINT, &disc, &plan, &[], &overrides).unwrap();
        assert_eq!(
            remembered.titles,
            vec![
                RememberedTitle {
                    source_file: Some("00800.mpls".to_string()),
                    dvd_title: None,
                    segment_map: "1,2,3".to_string(),
                    seconds: 7122,
                    role: TitleRole::Ignored,
                    audio: vec![],
                    subtitles: vec![],
                },
                RememberedTitle {
                    source_file: Some("00801.mpls".to_string()),
                    dvd_title: None,
                    segment_map: "1,2,4".to_string(),
                    seconds: 7570,
                    role: TitleRole::Feature,
                    audio: vec![],
                    subtitles: vec![],
                },
            ]
        );
    }

    #[test]
    fn planned_episodes_are_remembered_with_their_season_the_owners_season_filling_a_missing_one() {
        let disc = office_disc();
        let plan = plan_of(
            &disc,
            Kind::Tv,
            Some(OFFICE),
            &[
                (0, episode_role(Some(3), &[15], Some("Example Fifteen"))),
                (3, episode_role(None, &[12, 13], None)),
            ],
        );
        let overrides = Overrides { season: Some(3), ..Overrides::default() };
        let remembered = remember(FINGERPRINT, &disc, &plan, &[], &overrides).unwrap();
        assert_eq!(remembered.tmdb, OFFICE);
        assert_eq!(
            remembered.titles,
            vec![
                RememberedTitle {
                    source_file: Some("00001.mpls".to_string()),
                    dvd_title: None,
                    segment_map: "1".to_string(),
                    seconds: 1297,
                    role: TitleRole::Episodes { season: 3, episodes: vec![15] },
                    audio: vec![],
                    subtitles: vec![],
                },
                RememberedTitle {
                    source_file: Some("00004.mpls".to_string()),
                    dvd_title: None,
                    segment_map: "4".to_string(),
                    seconds: 2585,
                    role: TitleRole::Episodes { season: 3, episodes: vec![12, 13] },
                    audio: vec![],
                    subtitles: vec![],
                },
            ]
        );
    }

    #[test]
    fn an_episode_without_a_season_cannot_be_remembered() {
        let disc = office_disc();
        let plan = plan_of(&disc, Kind::Tv, Some(OFFICE), &[(0, episode_role(None, &[15], None))]);
        assert_eq!(
            remember(FINGERPRINT, &disc, &plan, &[], &Overrides::default()),
            Err("planned title 0 (t00-00001) has no season and episode numbers to remember: assign them first \
                 (spec 8.5)"
                .to_string())
        );
    }

    #[test]
    fn a_job_without_any_identity_cannot_be_remembered() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, None, &[(0, feature_role(None))]);
        assert_eq!(
            remember(FINGERPRINT, &disc, &plan, &[], &Overrides::default()),
            Err("the job has no TMDb identity to remember: set its identity first (spec 11 Remember)".to_string())
        );
    }

    #[test]
    fn the_owners_set_identity_is_the_identity_remembered() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, None, &[(0, feature_role(None))]);
        let chosen = TmdbIdentity { kind: TmdbKind::Movie, id: 54321 };
        let overrides = Overrides { identity: Some(chosen), ..Overrides::default() };
        let remembered = remember(FINGERPRINT, &disc, &plan, &[], &overrides).unwrap();
        assert_eq!(remembered.tmdb, chosen);
        assert_eq!(remembered.fingerprint, FINGERPRINT);
    }

    #[test]
    fn each_title_keeps_its_own_overrides_for_a_signature_two_titles_share() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[(0, feature_role(Some("Theatrical Cut"))), (1, feature_role(Some("Unrated Cut")))],
        );
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decisions = [decided(&plan, &disc, 0, &record, None), decided(&plan, &disc, 1, &record, None)];
        let remembered = remember(FINGERPRINT, &disc, &plan, &decisions, &Overrides::default()).unwrap();
        assert_eq!(remembered.titles.len(), 2);
        for title in &remembered.titles {
            assert_eq!(title.audio, vec![AudioOverrideEntry {
                signature: sig(StreamKind::Audio, "eng", Family::DtsHdMa, Some(6)),
                action: AudioOverride::Keep { order: 0, default: true },
            }]);
            assert_eq!(title.subtitles.len(), 1);
        }
    }

    #[test]
    fn a_decision_that_is_not_a_title_of_the_plan_cannot_be_remembered() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(None))]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let mut decision = decided(&plan, &disc, 0, &record, None);
        decision.title_key = "t09-00999".to_string();
        assert_eq!(
            remember(FINGERPRINT, &disc, &plan, &[decision], &Overrides::default()),
            Err("decision t09-00999 is not a title of the plan".to_string())
        );
    }

    #[test]
    fn a_title_without_a_duration_cannot_be_remembered() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(4, feature_role(None))]);
        assert_eq!(
            remember(FINGERPRINT, &disc, &plan, &[], &Overrides::default()),
            Err("title 4 has no duration (attribute 9) to remember".to_string())
        );
    }

    #[test]
    fn an_unnamed_cut_cannot_be_remembered_until_the_owner_names_it() {
        // Title 1 beside the "Theatrical Cut" is an unnamed cut: stored as Feature, the next job would plan it as a second
        // "Theatrical Cut" on the same path.
        let disc = film_disc();
        let plan =
            plan_of(&disc, Kind::Movie, Some(FILM), &[(0, feature_role(Some("Theatrical Cut"))), (1, unnamed_role())]);
        assert_eq!(
            remember(FINGERPRINT, &disc, &plan, &[], &Overrides::default()),
            Err("planned title 1 (t01-00801) is a cut without an edition name to remember: name it first (spec 8.4)"
                .to_string())
        );
        let overrides = Overrides {
            titles: vec![
                TitleOverrideEntry { title: 0, role: TitleRole::Cut { edition: "Theatrical Cut".to_string() } },
                TitleOverrideEntry { title: 1, role: TitleRole::Cut { edition: "Director's Cut".to_string() } },
            ],
            ..Overrides::default()
        };
        let remembered = remember(FINGERPRINT, &disc, &plan, &[], &overrides).unwrap();
        assert_eq!(
            remembered.titles.iter().map(|title| title.role.clone()).collect::<Vec<TitleRole>>(),
            vec![
                TitleRole::Cut { edition: "Theatrical Cut".to_string() },
                TitleRole::Cut { edition: "Director's Cut".to_string() },
            ]
        );
    }

    // The final review's fixes (one job's titles on one path, C3 and G3) and the per-title overrides of Remember (F10).

    /// Every title of `numbers` decided under `overrides`, then the job-level `same_path_reviews`.
    fn job_decided(plan: &Plan, disc: &Disc, numbers: &[u32], record: &TmdbRecord, overrides: Option<&Overrides>) -> Vec<FileDecision> {
        let mut decisions: Vec<FileDecision> =
            numbers.iter().map(|&number| decided(plan, disc, number, record, overrides)).collect();
        same_path_reviews(&mut decisions);
        decisions
    }

    /// The owner's Assign titles giving title 0 and title 1 these roles.
    fn assigned(first: TitleRole, second: TitleRole) -> Overrides {
        Overrides {
            titles: vec![TitleOverrideEntry { title: 0, role: first }, TitleOverrideEntry { title: 1, role: second }],
            ..Overrides::default()
        }
    }

    #[test]
    fn two_assigned_features_on_one_path_are_held_with_path_collision() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let overrides = assigned(TitleRole::Feature, TitleRole::Feature);
        let alone = [decided(&plan, &disc, 0, &record, Some(&overrides)), decided(&plan, &disc, 1, &record, Some(&overrides))];
        // Each title alone is filed automatically, and both build the one path of the theatrical cut.
        assert_eq!(path_of(&alone[0]), path_of(&alone[1]));
        assert!(alone.iter().all(|decision| decision.disposition == Disposition::AutoFile && decision.reviews.is_empty()));
        let decisions = job_decided(&plan, &disc, &[0, 1], &record, Some(&overrides));
        for decision in &decisions {
            assert_eq!(decision.reviews, vec![ReviewReason::PathCollision]);
            assert_eq!(decision.disposition, Disposition::NeedsReview);
        }
    }

    #[test]
    fn an_assigned_theatrical_cut_beside_a_feature_is_held_with_path_collision() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let overrides = assigned(TitleRole::Cut { edition: "Theatrical Cut".to_string() }, TitleRole::Feature);
        let decisions = job_decided(&plan, &disc, &[0, 1], &record, Some(&overrides));
        assert_eq!(path_of(&decisions[0]), path_of(&decisions[1]));
        for decision in &decisions {
            assert_eq!(decision.reviews, vec![ReviewReason::PathCollision]);
            assert_eq!(decision.disposition, Disposition::NeedsReview);
        }
    }

    #[test]
    fn two_structural_rivals_approved_without_names_build_one_path_and_are_held_with_path_collision() {
        // 8.3 rule 3's two rivals at 0.60 with multiple_features, approved as they stand: neither has an edition name, so
        // both file to the unnamed path and the job-level step holds them (spec 7: one cut would replace the other).
        let disc = film_disc();
        let mut plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        plan.title_confidence = 0.60;
        plan.reviews = vec![ReviewReason::MultipleFeatures];
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decisions = job_decided(&plan, &disc, &[0, 1], &record, None);
        assert_eq!(path_of(&decisions[0]), path_of(&decisions[1]));
        for decision in &decisions {
            assert_eq!(decision.reviews, vec![ReviewReason::MultipleFeatures, ReviewReason::PathCollision]);
            assert_eq!(decision.disposition, Disposition::NeedsReview);
        }
    }

    #[test]
    fn two_titles_planned_as_one_episode_are_held_with_path_collision() {
        let disc = office_disc();
        let plan = plan_of(
            &disc,
            Kind::Tv,
            Some(OFFICE),
            &[
                (0, episode_role(Some(3), &[15], Some("Example Fifteen"))),
                (1, episode_role(Some(3), &[15], Some("Example Fifteen"))),
            ],
        );
        let record = office_record();
        let decisions = job_decided(&plan, &disc, &[0, 1], &record, None);
        assert_eq!(path_of(&decisions[0]), path_of(&decisions[1]));
        for decision in &decisions {
            assert_eq!(decision.reviews, vec![ReviewReason::PathCollision]);
            assert_eq!(decision.disposition, Disposition::NeedsReview);
        }
    }

    #[test]
    fn two_cuts_with_their_own_edition_names_keep_their_paths_and_file_automatically() {
        let disc = film_disc();
        let plan = plan_of(
            &disc,
            Kind::Movie,
            Some(FILM),
            &[(0, feature_role(Some("Theatrical Cut"))), (1, feature_role(Some("Director's Cut")))],
        );
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let decisions = job_decided(&plan, &disc, &[0, 1], &record, None);
        assert_ne!(path_of(&decisions[0]), path_of(&decisions[1]));
        for decision in &decisions {
            assert!(decision.reviews.is_empty(), "{:?}", decision.reviews);
            assert_eq!(decision.disposition, Disposition::AutoFile);
        }
    }

    #[test]
    fn paths_that_differ_only_by_case_or_separator_are_one_path_and_a_decision_without_a_path_is_left_alone() {
        let disc = film_disc();
        let plan = plan_of(&disc, Kind::Movie, Some(FILM), &[(0, unnamed_role()), (1, unnamed_role())]);
        let record = film_record(12345, "Film", 2010, Some("tt0012345"));
        let mut decisions = [decided(&plan, &disc, 0, &record, None), decided(&plan, &disc, 1, &record, None)];
        let first = decisions[0].path.clone().unwrap();
        decisions[1].path = Some(NamedPath { path: first.path.to_uppercase().replace('\\', "/"), ..first });
        let mut pathless = decided(&plan, &disc, 0, &record, None);
        pathless.path = None;
        pathless.disposition = Disposition::NeedsReview;
        let before = pathless.clone();
        let mut job = vec![decisions[0].clone(), decisions[1].clone(), pathless];
        same_path_reviews(&mut job);
        assert!(job[0].reviews.contains(&ReviewReason::PathCollision));
        assert!(job[1].reviews.contains(&ReviewReason::PathCollision));
        assert_eq!(job[2], before);
    }

    /// An English Dolby Digital stereo track.
    const ENG_AC3_2CH: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (13, "192 Kb/s"), (14, "2")];
    /// An English derived forced-only PGS stream (flags 4096 and 2048).
    const ENG_PGS_FORCED_ONLY: &[(u32, &str)] =
        &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles"), (22, "6144")];

    /// A Blu-ray of two episode titles 0 and 1 with the same streams.
    fn two_episodes(streams: &[(u32, &[(u32, &str)])]) -> Disc {
        let t0: &[(u32, &str)] = &[(8, "6"), (9, "0:21:37"), (11, "4800000000"), (16, "00001.mpls"), (26, "1")];
        let t1: &[(u32, &str)] = &[(8, "6"), (9, "0:21:30"), (11, "4700000000"), (16, "00002.mpls"), (26, "2")];
        let text = scan_text(&[(1, "Blu-ray disc"), (32, "EXAMPLE SHOW S3D3")], &[(0, t0, streams), (1, t1, streams)]);
        disc(&parse(&text)).unwrap()
    }

    /// The plan of `two_episodes`: title 0 episode 15, title 1 episode 16 of season 3.
    fn two_episode_plan(disc: &Disc) -> Plan {
        plan_of(
            disc,
            Kind::Tv,
            Some(OFFICE),
            &[(0, episode_role(Some(3), &[15], Some("Example Fifteen"))), (1, episode_role(Some(3), &[16], Some("Example Sixteen")))],
        )
    }

    #[test]
    fn a_remembered_job_reproduces_each_titles_reviewed_audio_when_two_titles_disagree() {
        // F10: both episodes hold English DTS-HD MA 5.1 and English Dolby Digital 2.0. In review the owner marks title 0's
        // stereo track as commentary and keeps title 1's, then chooses Remember: each title keeps its own decision.
        let disc = two_episodes(&[(0, HD_VIDEO), (1, ENG_DTS_HD_MA_6CH), (2, ENG_AC3_2CH)]);
        let plan = two_episode_plan(&disc);
        let record = office_record();
        let overrides = Overrides {
            audio: vec![AudioTrackOverride { title: 0, stream: 2, action: AudioOverride::Commentary }],
            ..Overrides::default()
        };
        let reviewed = [decided(&plan, &disc, 0, &record, Some(&overrides)), decided(&plan, &disc, 1, &record, Some(&overrides))];
        assert_eq!(reviewed[0].audio.dropped, vec![(2, AudioDrop::Reviewed)]);
        assert_eq!(reviewed[1].audio.kept.iter().map(|track| track.stream).collect::<Vec<u32>>(), vec![1, 2]);
        let remembered = remember(FINGERPRINT, &disc, &plan, &reviewed, &overrides).unwrap();
        let dts = sig(StreamKind::Audio, "eng", Family::DtsHdMa, Some(6));
        let stereo = sig(StreamKind::Audio, "eng", Family::Ac3, Some(2));
        assert_eq!(
            remembered.titles[0].audio,
            vec![
                AudioOverrideEntry { signature: dts.clone(), action: AudioOverride::Keep { order: 0, default: true } },
                AudioOverrideEntry { signature: stereo.clone(), action: AudioOverride::Commentary },
            ]
        );
        assert_eq!(
            remembered.titles[1].audio,
            vec![
                AudioOverrideEntry { signature: dts, action: AudioOverride::Keep { order: 0, default: true } },
                AudioOverrideEntry { signature: stereo, action: AudioOverride::Keep { order: 1, default: false } },
            ]
        );
        // The next job for the fingerprint, without the owner's overrides, decides each title as the owner did, at 1.00.
        let library = LibraryFacts::default();
        let settings = settings();
        let matched = matched_titles(&remembered, &disc);
        for (number, before) in [0, 1].into_iter().zip(&reviewed) {
            let mut next = inputs(&plan, &disc, number, Some(&record), &library, &settings);
            next.audio.remembered = Some(matched[&number].audio.as_slice());
            let again = decide(&next).unwrap();
            let streams = |decision: &FileDecision| decision.audio.kept.iter().map(|track| track.stream).collect::<Vec<u32>>();
            assert_eq!(streams(&again), streams(before), "title {number}");
            assert_eq!(again.audio.confidence, 1.00, "title {number}");
        }
    }

    #[test]
    fn a_remembered_job_reproduces_each_titles_subtitle_role_when_two_titles_disagree() {
        let disc = two_episodes(&[(0, HD_VIDEO), (1, ENG_AC3_6CH), (2, ENG_PGS)]);
        let plan = two_episode_plan(&disc);
        let record = office_record();
        let overrides = Overrides {
            subtitles: vec![SubtitleTrackOverride {
                title: 0,
                stream: 2,
                action: SubtitleOverride::Keep { role: SubtitleRole::Sdh },
            }],
            ..Overrides::default()
        };
        let reviewed = [decided(&plan, &disc, 0, &record, Some(&overrides)), decided(&plan, &disc, 1, &record, Some(&overrides))];
        let roles = |decision: &FileDecision| decision.subtitles.kept.iter().map(|track| track.role).collect::<Vec<_>>();
        assert_eq!(roles(&reviewed[0]), vec![SubtitleRole::Sdh]);
        assert_eq!(roles(&reviewed[1]), vec![SubtitleRole::Regular]);
        let remembered = remember(FINGERPRINT, &disc, &plan, &reviewed, &overrides).unwrap();
        let pgs = sig(StreamKind::Subtitles, "eng", Family::Pgs, None);
        assert_eq!(
            remembered.titles[0].subtitles,
            vec![SubtitleOverrideEntry { signature: pgs.clone(), action: SubtitleOverride::Keep { role: SubtitleRole::Sdh } }]
        );
        assert_eq!(
            remembered.titles[1].subtitles,
            vec![SubtitleOverrideEntry { signature: pgs, action: SubtitleOverride::Keep { role: SubtitleRole::Regular } }]
        );
        let library = LibraryFacts::default();
        let settings = settings();
        let matched = matched_titles(&remembered, &disc);
        for (number, before) in [0, 1].into_iter().zip(&reviewed) {
            let mut next = inputs(&plan, &disc, number, Some(&record), &library, &settings);
            next.subtitles.remembered = Some(matched[&number].subtitles.as_slice());
            let again = decide(&next).unwrap();
            assert_eq!(roles(&again), roles(before), "title {number}");
            assert_eq!(again.subtitles.confidence, 1.00, "title {number}");
        }
    }

    #[test]
    fn a_forced_only_stream_empty_in_one_title_and_holding_captions_in_the_next_is_remembered_per_title() {
        // Both episodes hold an English PGS stream (2) and its derived forced-only stream (3). In title 0 the forced-only
        // stream is empty and MakeMKV removed it (MSG:4047); in title 1 it holds the forced captions. Flags are not part
        // of the signature, so stream 3 is (subtitles, eng, PGS, position 1) in both titles, each title keeping its own.
        let disc = two_episodes(&[(0, HD_VIDEO), (1, ENG_AC3_6CH), (2, ENG_PGS), (3, ENG_PGS_FORCED_ONLY)]);
        let plan = two_episode_plan(&disc);
        let record = office_record();
        let library = LibraryFacts::default();
        let settings = settings();
        let dialogue: Vec<f64> = (0..1000).map(|k| 10.0 + 6.0 * f64::from(k)).collect();
        let forced: Vec<f64> = (0..20).map(|k| 100.0 + 60.0 * f64::from(k)).collect();
        let mut first = inputs(&plan, &disc, 0, Some(&record), &library, &settings);
        first.subtitles.events = BTreeMap::from([(2, dialogue.clone())]);
        first.subtitles.removed = [3].into_iter().collect();
        let mut second = inputs(&plan, &disc, 1, Some(&record), &library, &settings);
        second.subtitles.events = BTreeMap::from([(2, dialogue.clone()), (3, forced.clone())]);
        let decisions = [decide(&first).unwrap(), decide(&second).unwrap()];
        assert_eq!(decisions[0].subtitles.dropped, vec![(3, SubtitleDrop::DerivedForcedOnlyEmpty)]);
        assert!(decisions[1].subtitles.kept.iter().any(|track| track.stream == 3 && track.role == SubtitleRole::Forced));
        let remembered = remember(FINGERPRINT, &disc, &plan, &decisions, &Overrides::default()).unwrap();
        let regular = SubtitleOverrideEntry {
            signature: sig(StreamKind::Subtitles, "eng", Family::Pgs, None),
            action: SubtitleOverride::Keep { role: SubtitleRole::Regular },
        };
        let forced_only = StreamSignature { position: 1, ..sig(StreamKind::Subtitles, "eng", Family::Pgs, None) };
        assert_eq!(
            remembered.titles[0].subtitles,
            vec![regular.clone(), SubtitleOverrideEntry { signature: forced_only.clone(), action: SubtitleOverride::Drop }]
        );
        assert!(remembered.titles[1].subtitles.contains(&SubtitleOverrideEntry {
            signature: forced_only,
            action: SubtitleOverride::Keep { role: SubtitleRole::Forced },
        }));
        // The next job for the fingerprint keeps title 1's forced stream as the remembered decision says, at 1.00.
        let matched = matched_titles(&remembered, &disc);
        let mut next = inputs(&plan, &disc, 1, Some(&record), &library, &settings);
        next.subtitles.events = BTreeMap::from([(2, dialogue), (3, forced)]);
        next.subtitles.remembered = Some(matched[&1].subtitles.as_slice());
        let again = decide(&next).unwrap();
        assert!(again.subtitles.kept.iter().any(|track| track.stream == 3 && track.role == SubtitleRole::Forced));
        assert_eq!(again.subtitles.confidence, 1.00);
    }
}
