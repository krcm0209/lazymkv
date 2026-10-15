//! `lazymkv-dev decide` over every recording of the evidence clone, compared section by section with the goldens
//! write_goldens.py wrote (port spec 3.4 and 5; Task 14): `reconcile` with `<label>/reconcile.json`, every `audio`
//! entry with `<label>/audio-<folder>.json`, `subtitles` with `<label>/subs.json`, and `reviews` with the review
//! reasons of those goldens. Floats agree within spec section 4's tolerance (structural_diff, Tolerance::spec()).
//! A comparison whose input is absent (no evidence clone, no golden/ folder yet, no level series, no OCR text, OCR
//! text other than the golden's) prints "skipped: <reason>" and a section with no golden prints "skipped: not
//! covered: ..."; neither fails, and neither passes silently. The audio comparison runs the measures again (Plain,
//! no cache), so with the level series present a recording takes up to a few minutes in the debug profile.
//! Inputs: the evidence clone (LAZYMKV_EVIDENCE, else corpus/owner under the repository root) and the OCR text
//! (LAZYMKV_OCR_TEXT, laid out as <DIR>/<slug>/<label>/<reading>/sub-<n>.ocr.jsonl).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lazymkv_core::audio::labels::is_slug;
use lazymkv_core::audio::report::FolderJson;
use lazymkv_core::recording::files::rip_labels;
use lazymkv_core::subtitle::text::SubsJson;
use lazymkv_dev::cmd::decide::{audio_reviews, decide_doc, subtitle_reviews, AudioEntry, SubtitleEntry};
use lazymkv_dev::evidence::{ocr_text_dir, resolve_evidence, resolve_recording, skip, Ctx, Recording};
use lazymkv_dev::pyjson::{structural_diff, Tolerance};
use serde_json::Value;

/// The recordings of the evidence clone, one test each. A recording added later is listed by
/// recordings_without_a_test_are_listed_as_not_covered until it gets its test here.
const RECORDINGS: [&str; 4] =
    ["pans-labyrinth-2006-uhd", "short-circuit-1986-dvd", "short-circuit-2-1988-dvd", "the-office-2005-s3d3-bluray"];
/// At most this many differences are reported per comparison.
const SHOWN: usize = 20;
/// How to give the OCR text, at the end of every no-text reason.
const TEXT_HINT: &str = "(give --text DIR or LAZYMKV_OCR_TEXT)";

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

/// decide over every rip label of `slug`, each section against its golden.
fn check_recording(slug: &str) {
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
        check_label(&ctx, &rec, label, &golden_dir.join(label), text_root.as_deref(), &mut failures);
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
/// its golden (a section that ran has its golden, and a golden whose input is here was compared).
fn check_label(ctx: &Ctx, rec: &Recording, label: &str, golden_label: &Path, text_root: Option<&Path>, failures: &mut Vec<String>) {
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
    let doc = match decide_doc(ctx, rec, label, text.as_deref(), None) {
        Ok(doc) => doc,
        Err(refused) => {
            failures.push(format!("{}/{}: decide exited {}: {}", rec.slug, label, refused.code, refused.line));
            return;
        }
    };
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
                    comparable = false;
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
}

#[test]
fn pans_labyrinth_2006_uhd() {
    check_recording("pans-labyrinth-2006-uhd");
}

#[test]
fn short_circuit_1986_dvd() {
    check_recording("short-circuit-1986-dvd");
}

#[test]
fn short_circuit_2_1988_dvd() {
    check_recording("short-circuit-2-1988-dvd");
}

#[test]
fn the_office_2005_s3d3_bluray() {
    check_recording("the-office-2005-s3d3-bluray");
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
