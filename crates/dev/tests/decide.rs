//! `lazymkv-dev decide` over every recording of the evidence clone, compared section by section with the goldens
//! write_goldens.py wrote (port spec 3.4 and 5; Task 14), and its phase 1 sections `plan` and `decision` (RP14, design
//! spec 6.4 step 11) with the expectations below, read from each recording's files: `reconcile` with
//! `<label>/reconcile.json`, every `audio`
//! entry with `<label>/audio-<folder>.json`, `subtitles` with `<label>/subs.json`, and `reviews` with the review
//! reasons of those goldens. Floats agree within spec section 4's tolerance (structural_diff, Tolerance::spec()).
//! A comparison whose input is absent (no evidence clone, no golden/ folder yet, no level series, no OCR text, OCR
//! text other than the golden's) prints "skipped: <reason>" and a section with no golden prints "skipped: not
//! covered: ..."; neither fails, and neither passes silently. The audio comparison runs the measures again (Plain,
//! no cache), so with the level series present a recording takes up to a few minutes in the debug profile.
//! Inputs: the evidence clone (LAZYMKV_EVIDENCE, else corpus/owner under the repository root) and the OCR text
//! (LAZYMKV_OCR_TEXT, laid out as <DIR>/<slug>/<label>/<reading>/sub-<n>.ocr.jsonl).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::path::{Path, PathBuf};

use lazymkv_core::audio::dataset::LEVELS_NOT_HERE;
use lazymkv_core::audio::labels::is_slug;
use lazymkv_core::audio::report::FolderJson;
use lazymkv_core::decision::Disposition;
use lazymkv_core::disc::{disc, Disc};
use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
use lazymkv_core::lookup::{IdentityStep, Kind};
use lazymkv_core::plan::{title_key, Mode};
use lazymkv_core::recording::files::rip_labels;
use lazymkv_core::recording::{decode_text, Encoding, Errors};
use lazymkv_core::review::ReviewReason;
use lazymkv_core::robot::parse;
use lazymkv_core::subtitle::labels::TEXT_HINT;
use lazymkv_core::subtitle::text::SubsJson;
use lazymkv_core::tracks::audio::AudioDrop;
use lazymkv_core::tracks::subtitle::SubtitleDrop;
use lazymkv_core::tracks::SubtitleRole;
use lazymkv_dev::cmd::decide::{
    audio_reviews, decide_doc, read_decide_inputs, subtitle_reviews, AudioEntry, DecideDoc, DecideInputs, DecisionEntry,
    PlanEntry, SubtitleEntry,
};
use lazymkv_dev::cmd::plan::PlanOptions;
use lazymkv_dev::evidence::{ocr_text_dir, resolve_evidence, resolve_recording, skip, Ctx, Recording};
use lazymkv_dev::pyjson::{structural_diff, Tolerance};
use serde_json::Value;

mod clone;
use clone::episode_names;

/// The recordings of the evidence clone, one test each. A recording added later is listed by
/// recordings_without_a_test_are_listed_as_not_covered until it gets its test here.
const RECORDINGS: [&str; 4] =
    ["pans-labyrinth-2006-uhd", "short-circuit-1986-dvd", "short-circuit-2-1988-dvd", "the-office-2005-s3d3-bluray"];
/// At most this many differences are reported per comparison.
const SHOWN: usize = 20;

/// The context of a test run: the process's cwd (crates/dev under cargo test), no environment but the two
/// variables these tests read.
fn context() -> Ctx {
    let mut ctx = Ctx::for_test(&std::env::current_dir().expect("a current directory"));
    for name in ["LAZYMKV_EVIDENCE", "LAZYMKV_OCR_TEXT"] {
        if let Ok(value) = std::env::var(name) {
            ctx.env.insert(name.to_string(), value);
        }
    }
    ctx
}

fn to_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("decide's sections serialise")
}

/// The golden at `path`, None when it is not there; Err when it cannot be read or is not JSON.
fn golden(path: &Path) -> Result<Option<Value>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {}", path.display(), error))?;
    serde_json::from_str(&text).map(Some).map_err(|error| format!("{}: {}", path.display(), error))
}

/// The differences between a golden and decide's section, named, at most SHOWN of them.
fn diff_into(name: &str, expected: &Value, actual: &Value, failures: &mut Vec<String>) {
    let diff = structural_diff(expected, actual, &Tolerance::spec());
    for line in diff.iter().take(SHOWN) {
        failures.push(format!("{}: {}", name, line));
    }
    if diff.len() > SHOWN {
        failures.push(format!("{}: ... and {} more differences", name, diff.len() - SHOWN));
    }
}

/// Whether a level folder's skip reason is the one missing-input case: the folder's level files are not in the
/// clone (git keeps only streams.json), "<slug>/<label>/<folder>: " + LEVELS_NOT_HERE. Any other reason where the
/// golden exists is a regression of the port.
fn levels_not_here(skipped: &str) -> bool {
    skipped.ends_with(LEVELS_NOT_HERE)
}

/// decide over every rip label of `slug`, each section against its golden, then the plan and decision sections against
/// `expected` (RP14).
fn check_recording(slug: &str, expected: &RecordingExpectation) {
    let ctx = context();
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(dir) => dir,
        Err(refused) => return skip(&refused.line),
    };
    let evidence_arg = evidence.to_string_lossy().into_owned();
    let rec = match resolve_recording(&ctx, Some(evidence_arg.as_str()), slug) {
        Ok(rec) => rec,
        Err(refused) => return skip(&refused.line),
    };
    let golden_dir = evidence.join(slug).join("golden");
    if !golden_dir.is_dir() {
        return skip(&format!("no golden/ folder for {}", slug));
    }
    let text_root = ocr_text_dir(&ctx, None);
    let labels: Vec<String> = rip_labels(&rec.tree, &rec.dir).into_iter().map(|rip| rip.label).collect();
    let mut failures = Vec::new();
    for label in &labels {
        check_label(&ctx, &rec, label, &golden_dir.join(label), text_root.as_deref(), expected, &mut failures);
    }
    // Every label with an expectation is a rip label of the recording.
    for (label, _) in &expected.labels {
        if !labels.iter().any(|found| found == label) {
            failures.push(format!("{}/{}: has an expectation but is not a rip label of the recording", slug, label));
        }
    }
    // Every golden reconcile.json belongs to a rip label decide was run on.
    match std::fs::read_dir(&golden_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().join("reconcile.json").is_file() && !labels.contains(&name) {
                    failures.push(format!("{}/golden/{}/reconcile.json: {} is not a rip label of the recording", slug, name, name));
                }
            }
        }
        Err(error) => failures.push(format!("{}: {}", golden_dir.display(), error)),
    }
    assert!(failures.is_empty(), "decide differs from the goldens of {}:\n{}", slug, failures.join("\n"));
}

/// One rip label: reconcile, every audio entry, the subtitles, then the reviews when every section lines up with
/// its golden (a section that ran has its golden, and a golden whose input is here was compared); then the plan and
/// decision sections of the same document against the recording's expectations.
fn check_label(
    ctx: &Ctx,
    rec: &Recording,
    label: &str,
    golden_label: &Path,
    text_root: Option<&Path>,
    expected: &RecordingExpectation,
    failures: &mut Vec<String>,
) {
    let name = |file: &str| format!("{}/golden/{}/{}", rec.slug, label, file);
    let subs_golden = match golden(&golden_label.join("subs.json")) {
        Ok(found) => found,
        Err(error) => {
            failures.push(error);
            None
        }
    };
    // decide reads the OCR text the golden was read from (<DIR>/<slug>/<label>/<reading>/) as its --text, so its
    // reading is the golden's; without a subs golden it looks under <DIR> itself.
    let text: Option<PathBuf> = match (&subs_golden, text_root) {
        (Some(expected), Some(root)) => {
            let reading = expected.get("reading").and_then(Value::as_str).unwrap_or_default();
            let dir = root.join(&rec.slug).join(label).join(reading);
            if dir.is_dir() {
                Some(dir)
            } else {
                skip(&format!("no OCR text: {} {}", dir.display(), TEXT_HINT));
                None
            }
        }
        (Some(_), None) => {
            skip(&format!("no OCR text for {}/{} {}", rec.slug, label, TEXT_HINT));
            None
        }
        (None, root) => root.map(Path::to_path_buf),
    };
    let inputs = match read_decide_inputs(ctx, rec, label, text.as_deref(), None, &PlanOptions::default()) {
        Ok(inputs) => inputs,
        Err(refused) => {
            failures.push(format!("{}/{}: decide exited {}: {}", rec.slug, label, refused.code, refused.line));
            return;
        }
    };
    let doc = decide_doc(&inputs);
    let mut reviews: BTreeSet<&'static str> = BTreeSet::new();
    let mut comparable = true;

    match golden(&golden_label.join("reconcile.json")) {
        Ok(Some(expected)) => diff_into(&name("reconcile.json"), &expected, &to_value(&doc.reconcile), failures),
        Ok(None) => skip(&format!("not covered: {} (no golden)", name("reconcile.json"))),
        Err(error) => failures.push(error),
    }

    let mut seen = BTreeSet::new();
    for entry in &doc.audio {
        match entry {
            AudioEntry::Results(result) => {
                let file = format!("audio-{}.json", result.folder);
                seen.insert(file.clone());
                match golden(&golden_label.join(&file)) {
                    Ok(Some(expected)) => {
                        diff_into(&name(&file), &expected, &to_value(result), failures);
                        match serde_json::from_value::<FolderJson>(expected) {
                            Ok(expected) => reviews.extend(audio_reviews(&expected)),
                            Err(error) => failures.push(format!("{}: not a FolderJson: {}", name(&file), error)),
                        }
                    }
                    Ok(None) => {
                        skip(&format!("not covered: {} (no golden)", name(&file)));
                        comparable = false;
                    }
                    Err(error) => failures.push(error),
                }
            }
            AudioEntry::Skipped { folder, skipped } => {
                let file = format!("audio-{}.json", folder);
                skip(skipped);
                if golden_label.join(&file).is_file() {
                    if levels_not_here(skipped) {
                        comparable = false;
                    } else {
                        failures.push(format!(
                            "{}: decide skipped the level folder ({}) where the golden measured it",
                            name(&file),
                            skipped
                        ));
                    }
                }
                seen.insert(file);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(golden_label) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.starts_with("audio-") && file.ends_with(".json") && !seen.contains(&file) {
                failures.push(format!("{}: decide gives no entry for this level folder", name(&file)));
            }
        }
    }

    match (&doc.subtitles, subs_golden) {
        (SubtitleEntry::Results(subs), Some(expected)) => {
            let actual = to_value(subs);
            if expected.get("text_sha1") != actual.get("text_sha1") {
                let shown = text.as_deref().map(|dir| dir.display().to_string()).unwrap_or_default();
                skip(&format!(
                    "{}: the OCR text under {} is not the text the golden was read from (text_sha1 differs)",
                    name("subs.json"),
                    shown
                ));
                comparable = false;
            } else {
                diff_into(&name("subs.json"), &expected, &actual, failures);
                match serde_json::from_value::<SubsJson>(expected) {
                    Ok(expected) => reviews.extend(subtitle_reviews(&expected)),
                    Err(error) => failures.push(format!("{}: not a SubsJson: {}", name("subs.json"), error)),
                }
            }
        }
        (SubtitleEntry::Results(_), None) => {
            skip(&format!("not covered: {} (no golden)", name("subs.json")));
            comparable = false;
        }
        (SubtitleEntry::Skipped { skipped }, Some(expected)) => {
            let golden_read = expected.get("exit_code").and_then(Value::as_i64) == Some(0);
            if golden_read && text.is_some() {
                failures.push(format!(
                    "{}: decide skipped the subtitles ({}) where the golden read them",
                    name("subs.json"),
                    skipped
                ));
            } else {
                skip(&format!("{}/{}: subtitles: {}", rec.slug, label, skipped));
                if golden_read {
                    comparable = false;
                }
            }
        }
        (SubtitleEntry::Skipped { skipped }, None) => skip(&format!("{}/{}: subtitles: {}", rec.slug, label, skipped)),
    }

    if comparable {
        let expected: Vec<String> = reviews.into_iter().map(str::to_string).collect();
        if doc.reviews != expected {
            failures.push(format!("{}/{}: reviews {:?}, the goldens give {:?}", rec.slug, label, doc.reviews, expected));
        }
    } else {
        skip(&format!(
            "{}/{}: reviews not compared: a section ran without its golden, or a golden's input is not here",
            rec.slug, label
        ));
    }

    check_plan(&rec.slug, label, &doc, &expected.plan, failures);
    match expected.labels.iter().find(|(name, _)| *name == label) {
        Some((_, expectation)) => check_decision(&inputs, rec, label, &doc, expectation, failures),
        None => skip(&format!(
            "not covered: {}/{}: no plan and decision expectation in crates/dev/tests/decide.rs",
            rec.slug, label
        )),
    }
}

/// The library roots of the second decision every label gets (the owner's share, spec 7 and 14), so that its final
/// path and disposition show: decide's own document has spec 14's defaults, which set no root, so it builds no path
/// and holds every job (Task 13).
const MOVIES_ROOT: &str = r"\\unas-pro\media\Movies";
const SHOWS_ROOT: &str = r"\\unas-pro\media\Shows";
/// The review reasons only the content rule's verdicts raise (8.6 "Content", RP7): compared only when every level
/// folder of the label was measured.
const VERDICT_REVIEWS: [ReviewReason; 2] = [ReviewReason::CommentaryDetected, ReviewReason::AudioReferenceUnclear];

/// What decide's plan and decision sections must hold for one recording: the plan, and the decision per rip label.
struct RecordingExpectation {
    plan: PlanExpectation,
    labels: Vec<(&'static str, LabelExpectation)>,
}

/// The plan section: identity step, identity confidence and TMDb identity (8.1), kind (8.2), titles in plan order with
/// their keys (spec 5), title confidence (8.3, 8.5), mode (6.1 step 5) and review reasons.
struct PlanExpectation {
    step: IdentityStep,
    identity_confidence: f64,
    tmdb: Option<TmdbIdentity>,
    kind: Kind,
    titles: Vec<(u32, String)>,
    title_confidence: f64,
    mode: Mode,
    reviews: Vec<ReviewReason>,
}

/// The decision section of one rip label: skipped with this reason, or decided.
enum LabelExpectation {
    Skipped(&'static str),
    Decided(DecisionExpectation),
}

/// A decided rip label. The lists of scan streams are sorted; `audio_kept` is in final order (8.6 step 5),
/// `subtitles_kept` in 8.10's order; the drops are compared sorted by stream.
struct DecisionExpectation {
    title_key: String,
    md5_streams: Vec<u32>,
    profile_streams: Vec<u32>,
    event_streams: Vec<u32>,
    verdict_streams: Vec<u32>,
    verdict_reference: Option<u32>,
    removed: Vec<u32>,
    audio_kept: Vec<u32>,
    audio_dropped: Vec<(u32, AudioDrop)>,
    audio_reviews: Vec<ReviewReason>,
    subtitles_kept: Vec<(u32, SubtitleRole)>,
    subtitles_dropped: Vec<(u32, SubtitleDrop)>,
    /// Identity, title and track confidence.
    confidences: (f64, f64, f64),
    reviews: Vec<ReviewReason>,
    /// The final path and the disposition under MOVIES_ROOT and SHOWS_ROOT.
    path: Option<String>,
    disposition: Disposition,
}

/// Pan's Labyrinth (UHD, Spanish): TheDiscDB by fingerprint, TMDb movie 1417, 8.3 rule 1 on title 0. Its rehearsal rip
/// is title 1 (00055.mpls, 5 minutes 40 seconds), which the plan leaves out.
fn pans_labyrinth() -> RecordingExpectation {
    let path = r"\\unas-pro\media\Movies\Pan's Labyrinth (2006) {tmdb-1417}\Pan's Labyrinth (2006) {tmdb-1417} - 2160p.mkv"
        .to_string();
    RecordingExpectation {
        plan: PlanExpectation {
            step: IdentityStep::DiscdbOne,
            identity_confidence: 0.95,
            tmdb: Some(TmdbIdentity { kind: TmdbKind::Movie, id: 1417 }),
            kind: Kind::Movie,
            titles: vec![(0, "t00-00800".to_string())],
            title_confidence: 0.98,
            mode: Mode::Direct,
            reviews: Vec::new(),
        },
        labels: vec![
            (
                "all-tracks",
                LabelExpectation::Decided(DecisionExpectation {
                    title_key: "t00-00800".to_string(),
                    md5_streams: vec![1, 2, 3],
                    profile_streams: vec![1, 2],
                    event_streams: vec![4, 6],
                    verdict_streams: vec![2, 3],
                    verdict_reference: Some(1),
                    removed: vec![5, 7],
                    // The Spanish original first (D9), then the English stereo track, a cross-language commentary.
                    audio_kept: vec![1, 3],
                    audio_dropped: vec![(2, AudioDrop::Derived)],
                    audio_reviews: vec![ReviewReason::CommentaryDetected],
                    subtitles_kept: vec![(4, SubtitleRole::Regular)],
                    subtitles_dropped: vec![
                        (5, SubtitleDrop::DerivedForcedOnlyEmpty),
                        (6, SubtitleDrop::OtherLanguage),
                        (7, SubtitleDrop::OtherLanguage),
                    ],
                    confidences: (0.95, 0.98, 0.90),
                    reviews: vec![ReviewReason::CommentaryDetected],
                    path: Some(path.clone()),
                    disposition: Disposition::NeedsReview,
                }),
            ),
            (
                "eng-only",
                LabelExpectation::Decided(DecisionExpectation {
                    title_key: "t00-00800".to_string(),
                    md5_streams: vec![3],
                    profile_streams: Vec::new(),
                    event_streams: vec![4],
                    // audio-levels-20ms-1 measured stream 3 alone: no candidate, no verdict, no reference.
                    verdict_streams: Vec::new(),
                    verdict_reference: None,
                    removed: vec![5],
                    // The eng profile left the Spanish streams out: the original language is wanted but absent.
                    audio_kept: vec![3],
                    audio_dropped: Vec::new(),
                    audio_reviews: vec![ReviewReason::OriginalLanguageMissing],
                    subtitles_kept: vec![(4, SubtitleRole::Regular)],
                    subtitles_dropped: vec![(5, SubtitleDrop::DerivedForcedOnlyEmpty)],
                    confidences: (0.95, 0.98, 0.90),
                    reviews: vec![ReviewReason::OriginalLanguageMissing],
                    path: Some(path),
                    disposition: Disposition::NeedsReview,
                }),
            ),
            ("rehearsal", LabelExpectation::Skipped("the plan does not include title 1 (planned: 0)")),
        ],
    }
}

/// Short Circuit (DVD): no lookup answer, one structural rival, title 0; English 5.1 then a stereo track; no subtitle
/// stream. Without a TMDb record no path is built.
fn short_circuit() -> RecordingExpectation {
    RecordingExpectation {
        plan: PlanExpectation {
            step: IdentityStep::Nothing,
            identity_confidence: 0.0,
            tmdb: None,
            kind: Kind::Movie,
            titles: vec![(0, "t00-title01".to_string())],
            title_confidence: 0.85,
            mode: Mode::Direct,
            reviews: vec![ReviewReason::IdentityUnknown],
        },
        labels: vec![(
            "all-tracks",
            LabelExpectation::Decided(DecisionExpectation {
                title_key: "t00-title01".to_string(),
                md5_streams: vec![1, 2],
                profile_streams: Vec::new(),
                event_streams: Vec::new(),
                verdict_streams: vec![2],
                verdict_reference: Some(1),
                removed: Vec::new(),
                audio_kept: vec![1, 2],
                audio_dropped: Vec::new(),
                audio_reviews: Vec::new(),
                subtitles_kept: Vec::new(),
                subtitles_dropped: Vec::new(),
                confidences: (0.0, 0.85, 0.95),
                reviews: vec![ReviewReason::IdentityUnknown],
                path: None,
                disposition: Disposition::NeedsReview,
            }),
        )],
    }
}

/// Short Circuit 2 (DVD, volume label DVD_VIDEO): no lookup answer, title 0; English stereo kept, the other dubs
/// unwanted; the English VobSub stream regular and the closed captions SDH, the other subtitle languages dropped.
fn short_circuit_2() -> RecordingExpectation {
    let others = |streams: std::ops::RangeInclusive<u32>| -> Vec<(u32, SubtitleDrop)> {
        streams.map(|stream| (stream, SubtitleDrop::OtherLanguage)).collect()
    };
    RecordingExpectation {
        plan: PlanExpectation {
            step: IdentityStep::Nothing,
            identity_confidence: 0.0,
            tmdb: None,
            kind: Kind::Movie,
            titles: vec![(0, "t00-title01".to_string())],
            title_confidence: 0.85,
            mode: Mode::Direct,
            reviews: vec![ReviewReason::IdentityUnknown],
        },
        labels: vec![
            (
                "all-tracks-2",
                LabelExpectation::Decided(DecisionExpectation {
                    title_key: "t00-title01".to_string(),
                    // The manifest names stream 1 alone; the folder's audio-proxy.json holds all four audio streams.
                    md5_streams: vec![1, 2, 3, 4],
                    profile_streams: Vec::new(),
                    event_streams: vec![5, 6, 7, 8, 9, 10, 11, 12],
                    verdict_streams: Vec::new(),
                    verdict_reference: None,
                    removed: Vec::new(),
                    audio_kept: vec![1],
                    audio_dropped: vec![
                        (2, AudioDrop::UnwantedLanguage),
                        (3, AudioDrop::UnwantedLanguage),
                        (4, AudioDrop::UnwantedLanguage),
                    ],
                    audio_reviews: Vec::new(),
                    subtitles_kept: vec![(5, SubtitleRole::Regular), (12, SubtitleRole::Sdh)],
                    subtitles_dropped: others(6..=11),
                    confidences: (0.0, 0.85, 0.90),
                    // The rip logged three read errors at offset 1048576 (6.2 step 7).
                    reviews: vec![ReviewReason::IdentityUnknown, ReviewReason::ReadErrors],
                    path: None,
                    disposition: Disposition::NeedsReview,
                }),
            ),
            (
                "eng-only",
                LabelExpectation::Decided(DecisionExpectation {
                    title_key: "t00-title01".to_string(),
                    md5_streams: vec![1],
                    profile_streams: Vec::new(),
                    event_streams: vec![5, 12],
                    verdict_streams: Vec::new(),
                    verdict_reference: None,
                    removed: Vec::new(),
                    audio_kept: vec![1],
                    audio_dropped: Vec::new(),
                    audio_reviews: Vec::new(),
                    subtitles_kept: vec![(5, SubtitleRole::Regular), (12, SubtitleRole::Sdh)],
                    subtitles_dropped: Vec::new(),
                    confidences: (0.0, 0.85, 0.90),
                    reviews: vec![ReviewReason::IdentityUnknown],
                    path: None,
                    disposition: Disposition::NeedsReview,
                }),
            ),
        ],
    }
}

/// The Office season 3 disc 3 (Blu-ray): TheDiscDB by fingerprint, TMDb show 2316, mapped episodes. e16-stereo is title
/// 0 (S03E16), rehearsal title 1 (S03E15, the twin DTS-HD MA tracks), t3-all-tracks title 3 (S03E12-E13, whose
/// two-episode name has a slash that 8.9 removes). The title keys are built from the playlists of `disc`, the
/// recording's own scan, and the episode names in the file names from `names`, TheDiscDB's Episode items by source
/// file, both read when the test runs (RP21: which title is which playlist, and the episode names, stay in the clone).
fn the_office(disc: &Disc, names: &BTreeMap<String, String>) -> RecordingExpectation {
    // The titles in plan order (8.5: ordered by episode), and spec 5's key of each at its position.
    let order = [3, 2, 1, 0, 6, 5];
    let key = |title: u32| {
        let position = order.iter().position(|&number| number == title).expect("a title of the plan");
        title_key(position, disc.title(title).unwrap_or_else(|sentence| panic!("{sentence}")), disc.is_dvd)
    };
    let show = r"\\unas-pro\media\Shows\The Office (2005) {tmdb-2316}\Season 03\The Office (2005)";
    // The file name of title `title`, holding `episodes`: TheDiscDB's episode name of its playlist, with the slash 8.9
    // removes from a component.
    let file = |title: u32, episodes: &str| {
        let playlist = disc.title(title).ok().and_then(|found| found.source_file.clone()).expect("a title with a playlist");
        let name = names.get(&playlist).unwrap_or_else(|| panic!("TheDiscDB's answer names no episode for {playlist}"));
        format!("{show} - {episodes} - {}.mkv", name.replace('/', ""))
    };
    RecordingExpectation {
        plan: PlanExpectation {
            step: IdentityStep::DiscdbOne,
            identity_confidence: 0.95,
            tmdb: Some(TmdbIdentity { kind: TmdbKind::Tv, id: 2316 }),
            kind: Kind::Tv,
            titles: order.iter().map(|&title| (title, key(title))).collect(),
            title_confidence: 0.98,
            mode: Mode::Direct,
            reviews: Vec::new(),
        },
        labels: vec![
            (
                "e16-stereo",
                LabelExpectation::Decided(DecisionExpectation {
                    title_key: key(0),
                    md5_streams: vec![1, 2, 3],
                    profile_streams: vec![1, 2],
                    event_streams: vec![4],
                    verdict_streams: vec![2, 3],
                    verdict_reference: Some(1),
                    removed: vec![5],
                    // The stereo track has the commentary pattern: kept, and the job held (D15).
                    audio_kept: vec![1, 3],
                    audio_dropped: vec![(2, AudioDrop::Derived)],
                    audio_reviews: vec![ReviewReason::CommentaryDetected],
                    subtitles_kept: vec![(4, SubtitleRole::Regular)],
                    subtitles_dropped: vec![(5, SubtitleDrop::DerivedForcedOnlyEmpty)],
                    confidences: (0.95, 0.98, 0.90),
                    reviews: vec![ReviewReason::CommentaryDetected],
                    path: Some(file(0, "S03E16")),
                    disposition: Disposition::NeedsReview,
                }),
            ),
            (
                "rehearsal",
                LabelExpectation::Decided(DecisionExpectation {
                    title_key: key(1),
                    md5_streams: vec![1, 2, 3, 4],
                    profile_streams: vec![1, 2, 3, 4],
                    event_streams: vec![5],
                    verdict_streams: vec![2, 3, 4],
                    verdict_reference: Some(1),
                    removed: vec![6],
                    // The twin tracks: stream 3's packets equal stream 1's (D13), its core 4 is derived.
                    audio_kept: vec![1],
                    audio_dropped: vec![
                        (2, AudioDrop::Derived),
                        (3, AudioDrop::Duplicate { of: 1 }),
                        (4, AudioDrop::Derived),
                    ],
                    audio_reviews: Vec::new(),
                    subtitles_kept: vec![(5, SubtitleRole::Regular)],
                    subtitles_dropped: vec![(6, SubtitleDrop::DerivedForcedOnlyEmpty)],
                    confidences: (0.95, 0.98, 0.90),
                    reviews: Vec::new(),
                    path: Some(file(1, "S03E15")),
                    disposition: Disposition::AutoFile,
                }),
            ),
            (
                "t3-all-tracks",
                LabelExpectation::Decided(DecisionExpectation {
                    title_key: key(3),
                    md5_streams: vec![1, 2, 3],
                    profile_streams: vec![1, 2],
                    event_streams: vec![4],
                    verdict_streams: vec![2, 3],
                    verdict_reference: Some(1),
                    removed: vec![5],
                    // note-commentary on the stereo track is a note, not a review reason.
                    audio_kept: vec![1, 3],
                    audio_dropped: vec![(2, AudioDrop::Derived)],
                    audio_reviews: Vec::new(),
                    subtitles_kept: vec![(4, SubtitleRole::Regular)],
                    subtitles_dropped: vec![(5, SubtitleDrop::DerivedForcedOnlyEmpty)],
                    confidences: (0.95, 0.98, 0.90),
                    reviews: Vec::new(),
                    path: Some(file(3, "S03E12-E13")),
                    disposition: Disposition::AutoFile,
                }),
            ),
        ],
    }
}

/// Adds "<what>: <actual:?>, expected <expected:?>" to `failures` when the two differ.
fn same<T: PartialEq + Debug>(failures: &mut Vec<String>, what: &str, actual: T, expected: T) {
    if actual != expected {
        failures.push(format!("{}: {:?}, expected {:?}", what, actual, expected));
    }
}

/// `entries` sorted by scan stream (a stable sort: one entry per stream in practice).
fn sorted<T>(mut entries: Vec<(u32, T)>) -> Vec<(u32, T)> {
    entries.sort_by_key(|(stream, _)| *stream);
    entries
}

/// The plan section of one label's document against the recording's plan expectation.
fn check_plan(slug: &str, label: &str, doc: &DecideDoc, expected: &PlanExpectation, failures: &mut Vec<String>) {
    let at = format!("{}/{}: plan", slug, label);
    let plan = match &doc.plan {
        PlanEntry::Results(found) => &found.plan,
        PlanEntry::Skipped { skipped } => {
            failures.push(format!("{}: skipped ({})", at, skipped));
            return;
        }
    };
    same(failures, &format!("{at}: identity step"), plan.identity.step, expected.step);
    same(failures, &format!("{at}: identity confidence"), plan.identity_confidence, expected.identity_confidence);
    same(failures, &format!("{at}: TMDb identity"), plan.identity.tmdb, expected.tmdb);
    same(failures, &format!("{at}: kind"), plan.kind, expected.kind);
    let titles: Vec<(u32, String)> = plan.titles.iter().map(|planned| (planned.title, planned.key.clone())).collect();
    let want: Vec<(u32, String)> = expected.titles.iter().map(|(title, key)| (*title, key.to_string())).collect();
    same(failures, &format!("{at}: titles"), titles, want);
    same(failures, &format!("{at}: title confidence"), plan.title_confidence, expected.title_confidence);
    same(failures, &format!("{at}: mode"), plan.mode, expected.mode);
    same(failures, &format!("{at}: reviews"), plan.reviews.clone(), expected.reviews.clone());
}

/// The decision section of one label's document against its expectation, then a second rendering of the label's inputs
/// with the owner's library roots in the plan's settings for the final path and the disposition (decide's own document
/// plans without a settings file, so its decision has no path).
fn check_decision(
    inputs: &DecideInputs,
    rec: &Recording,
    label: &str,
    doc: &DecideDoc,
    expected: &LabelExpectation,
    failures: &mut Vec<String>,
) {
    let at = format!("{}/{}: decision", rec.slug, label);
    let (want, got) = match (expected, &doc.decision) {
        (LabelExpectation::Skipped(why), DecisionEntry::Skipped { skipped }) => {
            same(failures, &format!("{at}: skipped"), skipped.as_str(), *why);
            return;
        }
        (LabelExpectation::Skipped(why), DecisionEntry::Results(_)) => {
            failures.push(format!("{at}: decided, expected it skipped with \"{why}\""));
            return;
        }
        (LabelExpectation::Decided(_), DecisionEntry::Skipped { skipped }) => {
            failures.push(format!("{at}: skipped ({skipped}), expected a decision"));
            return;
        }
        (LabelExpectation::Decided(want), DecisionEntry::Results(got)) => (want, got),
    };
    // Every level folder keeps its audio-proxy.json in git, so the label's MD5s are here whenever a level folder is.
    let levels = !doc.audio.is_empty();
    // A level folder whose level series are absent was skipped: no verdict.
    let measured = levels && doc.audio.iter().all(|entry| matches!(entry, AudioEntry::Results(_)));
    // OCR text was read and the decision used it: the text may add subtitle_ambiguous (8.7 "Text"). The decision uses it
    // only where 6.4 step 10 runs (two or more English or no-language subtitle streams in the file), and the facts
    // section lists the text streams only then, so a title with one English track is compared with OCR text too.
    let text_used = !got.facts.text_streams.is_empty();
    // Without the verdicts, the review reasons they raise are left out of both sides.
    let judged = |reasons: &[ReviewReason]| -> Vec<ReviewReason> {
        reasons.iter().copied().filter(|reason| measured || !VERDICT_REVIEWS.contains(reason)).collect()
    };
    let decision = &got.decision;
    same(failures, &format!("{at}: title key"), got.title_key.as_str(), want.title_key.as_str());
    same(failures, &format!("{at}: verify mismatches"), got.verify.mismatches.clone(), Vec::new());
    same(failures, &format!("{at}: verify ok"), got.verify.ok, true);
    same(failures, &format!("{at}: profile streams"), got.facts.profile_streams.clone(), want.profile_streams.clone());
    same(failures, &format!("{at}: event streams"), got.facts.event_streams.clone(), want.event_streams.clone());
    same(failures, &format!("{at}: removed streams"), got.facts.removed.clone(), want.removed.clone());
    let kept: Vec<(u32, SubtitleRole)> = decision.subtitles.kept.iter().map(|track| (track.stream, track.role)).collect();
    same(failures, &format!("{at}: subtitles kept"), kept, want.subtitles_kept.clone());
    same(
        failures,
        &format!("{at}: subtitles dropped"),
        sorted(decision.subtitles.dropped.clone()),
        want.subtitles_dropped.clone(),
    );
    same(
        failures,
        &format!("{at}: identity and title confidence"),
        (decision.confidences.identity, decision.confidences.title),
        (want.confidences.0, want.confidences.1),
    );
    // decide's own document has no library root.
    same(failures, &format!("{at}: path without a library root"), decision.path.clone(), None);
    same(failures, &format!("{at}: disposition without a library root"), decision.disposition, Disposition::NeedsReview);
    if levels {
        same(failures, &format!("{at}: MD5 streams"), got.facts.md5_streams.clone(), want.md5_streams.clone());
        let audio: Vec<u32> = decision.audio.kept.iter().map(|track| track.stream).collect();
        same(failures, &format!("{at}: audio kept"), audio, want.audio_kept.clone());
        same(failures, &format!("{at}: audio dropped"), sorted(decision.audio.dropped.clone()), want.audio_dropped.clone());
        same(failures, &format!("{at}: audio reviews"), judged(&decision.audio.reviews), judged(&want.audio_reviews));
    } else {
        skip(&format!(
            "{}/{}: decision's MD5s and audio tracks not compared: the label has no level folder",
            rec.slug, label
        ));
    }
    if measured {
        same(failures, &format!("{at}: verdict streams"), got.facts.verdict_streams.clone(), want.verdict_streams.clone());
        same(failures, &format!("{at}: verdict reference"), got.facts.verdict_reference, want.verdict_reference);
    } else {
        skip(&format!(
            "{}/{}: decision's verdicts, the review reasons they raise and the disposition not compared: a level \
             folder was not measured (its level series are not here)",
            rec.slug, label
        ));
    }
    if text_used {
        skip(&format!(
            "{}/{}: decision's track confidence, reviews and disposition not compared: the decision used OCR text, \
             and the text can add subtitle_ambiguous (8.7 \"Text\")",
            rec.slug, label
        ));
    } else {
        same(failures, &format!("{at}: track confidence"), decision.confidences.track, want.confidences.2);
        same(failures, &format!("{at}: reviews"), judged(&decision.reviews), judged(&want.reviews));
    }
    let PlanEntry::Results(_) = &doc.plan else {
        return;
    };
    let mut rooted = inputs.clone();
    if let PlanEntry::Results(plan) = &mut rooted.plan {
        plan.settings.movies_root = Some(MOVIES_ROOT.to_string());
        plan.settings.tv_root = Some(SHOWS_ROOT.to_string());
    }
    match decide_doc(&rooted).decision {
        DecisionEntry::Results(found) => {
            let path = found.decision.path.as_ref().map(|named| named.path.clone());
            same(failures, &format!("{at}: final path"), path, want.path.clone());
            if measured && !text_used {
                same(failures, &format!("{at}: disposition"), found.decision.disposition, want.disposition);
            }
        }
        DecisionEntry::Skipped { skipped } => failures.push(format!("{at}: with library roots: {skipped}")),
    }
}

/// The typed scan.txt of `slug` in the evidence clone, read when the test runs; None, with the skip line written, when
/// the clone or the recording is absent.
fn recorded_disc(slug: &str) -> Option<Disc> {
    let evidence = match resolve_evidence(&context(), None) {
        Ok(dir) => dir,
        Err(refused) => {
            skip(&refused.line);
            return None;
        }
    };
    let path = evidence.join(slug).join("scan.txt");
    if !path.is_file() {
        skip(&format!("no {} (this recording is absent)", path.display()));
        return None;
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {}", path.display(), error));
    let text = decode_text(&bytes, Encoding::Utf8Sig, Errors::Replace)
        .unwrap_or_else(|error| panic!("{}: {}", path.display(), error));
    Some(disc(&parse(&text)).unwrap_or_else(|sentence| panic!("{}: {}", path.display(), sentence)))
}

#[test]
fn pans_labyrinth_2006_uhd() {
    check_recording("pans-labyrinth-2006-uhd", &pans_labyrinth());
}

#[test]
fn short_circuit_1986_dvd() {
    check_recording("short-circuit-1986-dvd", &short_circuit());
}

#[test]
fn short_circuit_2_1988_dvd() {
    check_recording("short-circuit-2-1988-dvd", &short_circuit_2());
}

#[test]
fn the_office_2005_s3d3_bluray() {
    let slug = "the-office-2005-s3d3-bluray";
    let Some(disc) = recorded_disc(slug) else { return };
    let Ok(evidence) = resolve_evidence(&context(), None) else { return };
    check_recording(slug, &the_office(&disc, &episode_names(&evidence.join(slug))));
}

#[test]
fn recordings_without_a_test_are_listed_as_not_covered() {
    let ctx = context();
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(dir) => dir,
        Err(refused) => return skip(&refused.line),
    };
    let mut slugs: Vec<String> = std::fs::read_dir(&evidence)
        .expect("the evidence clone is readable")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_slug(name) && name != "golden")
        .collect();
    slugs.sort();
    for slug in slugs {
        if !RECORDINGS.contains(&slug.as_str()) {
            skip(&format!("not covered: {} (crates/dev/tests/decide.rs has no test for it)", slug));
        }
    }
}

#[test]
fn only_the_level_files_not_here_line_excuses_a_skipped_level_folder() {
    let absent = format!("disc/label/audio-levels-20ms-1-2: {}", LEVELS_NOT_HERE);
    assert!(levels_not_here(&absent));
    assert!(!levels_not_here("disc/label/audio-levels-20ms-1-2: manifest.txt does not begin with '# lazymkv audio-levels v2'"));
    assert!(!levels_not_here(""));
}
