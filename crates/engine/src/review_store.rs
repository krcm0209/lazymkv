//! The job's review answers in its job folder, `review.json` (phase 3 design, part 6.4): the merged `Overrides` of every
//! review and, for each review, its time, its action and the reasons shown. The file is under the engine's local data
//! folder, never a share. Its format and its `overrides` are read by `lazymkv_dev::cmd::plan::review_overrides`, the
//! one reading `--overrides FILE` also uses, so the engine's Decide and the relayed `plan --job` and `decide --job` take
//! the same answers from it. Def: P3-T7. Use: P3-T8, P3-T12, P3-T16.

use std::fs;
use std::io;
use std::path::Path;

use lazymkv_core::remembered::Overrides;
use lazymkv_core::review::ReviewReason;
use lazymkv_core::review_doc::{REVIEW_FILE, REVIEW_FORMAT};
use lazymkv_dev::cmd::plan::review_overrides;
use lazymkv_platform::write_atomic;
use serde_json::Value;

/// One review of the job (phase 3 design, part 6.4): when it was given (seconds since the Unix epoch), its action
/// (`approve`, ...) and the review reasons the document showed. Field order is the file's key order. Def: P3-T7.
/// Use: P3-T8, P3-T12, P3-T16.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Reviewed {
    pub time_unix: u64,
    pub action: String,
    pub reasons: Vec<ReviewReason>,
}

/// `review.json` (phase 3 design, part 6.4): `REVIEW_FORMAT`, the merged answers of every review, and the reviews in
/// the order they were given. Field order is the file's key order. Def: P3-T7. Use: P3-T8, P3-T12, P3-T16.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct ReviewFile {
    pub format: String,
    pub overrides: Overrides,
    pub reviews: Vec<Reviewed>,
}

/// `<job_dir>/review.json` read back (phase 3 design, parts 6.4, 6.5): Ok(None) when there is none; else its bytes (a
/// leading byte-order mark skipped, as `--overrides`' reader skips it) as JSON, its format and answers through
/// `review_overrides` (another format, or an answer key the strict reader does not know, is refused) and its `reviews`.
/// Every Err is `<path>: <reason>`. Def: P3-T7. Use: P3-T8, P3-T12, P3-T16.
pub(crate) fn read(job_dir: &Path) -> Result<Option<ReviewFile>, String> {
    let path = job_dir.join(REVIEW_FILE);
    let shown = path.display().to_string();
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{shown}: {error}")),
    };
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let value: Value = serde_json::from_slice(text).map_err(|error| format!("{shown}: {error}"))?;
    let overrides = review_overrides(&value, &shown)?;
    let reviews = value.get("reviews").cloned().unwrap_or(Value::Null);
    let reviews = serde_json::from_value(reviews).map_err(|error| format!("{shown}: reviews: {error}"))?;
    Ok(Some(ReviewFile { format: REVIEW_FORMAT.to_string(), overrides, reviews }))
}

/// One more review recorded (phase 3 design, part 6.4, Z5): the file as `read` gives it (none yet is an empty one; a
/// file `read` refuses is refused and left as it is), its answers merged with `answers` (`Overrides::merged`: a later
/// answer for the same title, stream or path replaces the earlier one, the rest stay), `review` appended, and the whole
/// written with `write_atomic`, pretty-printed with a final newline as the engine writes state.json. Gives what it wrote.
/// Every Err is `<path>: <reason>`. Def: P3-T7. Use: P3-T8, P3-T12, P3-T16.
pub(crate) fn record(job_dir: &Path, answers: &Overrides, review: Reviewed) -> Result<ReviewFile, String> {
    let path = job_dir.join(REVIEW_FILE);
    let earlier = read(job_dir)?;
    let mut file = earlier.unwrap_or_else(|| ReviewFile {
        format: REVIEW_FORMAT.to_string(),
        overrides: Overrides::default(),
        reviews: Vec::new(),
    });
    file.overrides = file.overrides.merged(answers);
    file.reviews.push(review);
    let mut bytes = serde_json::to_vec_pretty(&file).map_err(|error| format!("{}: {error}", path.display()))?;
    bytes.push(b'\n');
    write_atomic(&path, &bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use lazymkv_core::remembered::{Overrides, SubtitleOverride, SubtitleTrackOverride};
    use lazymkv_core::review::ReviewReason;
    use lazymkv_core::review_doc::{REVIEW_FILE, REVIEW_FORMAT};
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_core::tracks::SubtitleRole;

    /// The owner's answer for scan title 0's subtitle `stream`.
    fn subtitle(stream: u32, action: SubtitleOverride) -> SubtitleTrackOverride {
        SubtitleTrackOverride { title: 0, stream, action }
    }

    /// An approval at `time_unix` of a document that showed `subtitle_ambiguous`.
    fn approval(time_unix: u64) -> Reviewed {
        Reviewed { time_unix, action: "approve".to_string(), reasons: vec![ReviewReason::SubtitleAmbiguous] }
    }

    #[test]
    fn a_second_review_adds_to_the_first() {
        let s = Scratch::new("review-store-two");
        let first = Overrides { subtitles: vec![subtitle(5, SubtitleOverride::Drop)], ..Overrides::default() };
        let second = Overrides {
            subtitles: vec![
                subtitle(5, SubtitleOverride::Keep { role: SubtitleRole::Sdh }),
                subtitle(6, SubtitleOverride::Drop),
            ],
            ..Overrides::default()
        };
        let once = record(&s.0, &first, approval(100)).unwrap();
        assert_eq!(once.overrides, first);
        assert_eq!(once.reviews, vec![approval(100)]);
        let twice = record(&s.0, &second, approval(200)).unwrap();
        let expected = ReviewFile {
            format: REVIEW_FORMAT.to_string(),
            overrides: first.merged(&second),
            reviews: vec![approval(100), approval(200)],
        };
        assert_eq!(twice, expected);
        assert_eq!(read(&s.0), Ok(Some(expected)));
        // On disk: pretty-printed, the keys in the order format, overrides, reviews, and a review's own keys in order.
        let text = fs::read_to_string(s.0.join(REVIEW_FILE)).unwrap();
        assert!(text.starts_with("{\n  \"format\": \"lazymkv review v1\",\n  \"overrides\": {"), "{text}");
        assert!(text.ends_with("}\n"), "{text}");
        let at = |key: &str| text.find(key).unwrap();
        assert!(at("\"overrides\"") < at("\"reviews\""), "{text}");
        assert!(at("\"time_unix\"") < at("\"action\": \"approve\"") && at("\"action\": \"approve\"") < at("\"reasons\""));
        assert!(text.contains("\"subtitle_ambiguous\""), "{text}");
    }

    #[test]
    fn a_file_of_another_format_is_refused_naming_it() {
        let s = Scratch::new("review-store-format");
        let path = s.write_path(REVIEW_FILE, r#"{"format": "lazymkv review v9", "overrides": {}, "reviews": []}"#);
        let refusal = format!("{}: the format is \"lazymkv review v9\", not \"lazymkv review v1\"", path.display());
        assert_eq!(read(&s.0), Err(refusal.clone()));
        // A review is not recorded over it, and the file is left as it was.
        assert_eq!(record(&s.0, &Overrides::default(), approval(100)), Err(refusal));
        assert!(fs::read_to_string(&path).unwrap().contains("v9"));
        // Not JSON, and a misspelt answer key, are refused naming it too.
        s.write(REVIEW_FILE, "{");
        assert_eq!(read(&s.0), Err(format!("{}: EOF while parsing an object at line 1 column 1", path.display())));
        s.write(REVIEW_FILE, r#"{"format": "lazymkv review v1", "overrides": {"subtitle": []}, "reviews": []}"#);
        let unknown = "overrides: unknown key \"subtitle\" (identity, season, titles, audio, subtitles, collision)";
        assert_eq!(read(&s.0), Err(format!("{}: {unknown}", path.display())));
    }

    #[test]
    fn a_missing_file_is_no_review() {
        let s = Scratch::new("review-store-missing");
        assert_eq!(read(&s.0), Ok(None));
    }
}
