//! The review sheet's reader of subtitle lines (phase 3 design, part 6.2): the lines of a title's subtitle tracks that
//! step 10 read, taken at the three moments of ruling T32-R5 and keyed by scan stream. The text is held in memory only:
//! nothing here writes it, logs it or puts it in an error.
//!
//! `lazymkv-dev sheet <job folder> [--text DIR] [--held] [--all]` (phase 3 design, part 6.2; ruling T32-R8): the
//! review sheet of a job folder, for tests and for a wording trial; not an owner command. It reads the folder's
//! `state.json` and `scan.txt`, builds the document with `review_doc::build` and prints `review_doc::render` of it to
//! standard output. It looks at no share and writes nothing: no staged file, no collision and no library file is
//! known to it (`staged`, `collisions` and `nothing_at` are empty). `--text DIR` is the OCR reading, `DIR/sub-<n>.ocr.jsonl`,
//! and gives each subtitle stream its lines (`subtitle_lines` over each title's label folder of the job folder), with
//! the streams the label's `subs-ocr/summary.json` records as not read as `unread`; without it no line is shown.
//! `--held` sets the record's state to the disposition gate in memory, so that a filed job is shown as it was held;
//! `--all` gives every title an audio and a subtitles point. Exit 0; 2 for an argument error, a folder with no
//! readable `state.json` or `scan.txt`, or a job with nothing to review (`review_doc::build`'s refusal, named).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::Path;

use lazymkv_core::disc::{disc, Disc};
use lazymkv_core::job::{Held, State};
use lazymkv_core::record::{check_format, JobRecord, JOB_FORMAT};
use lazymkv_core::review_doc::{self, DocInputs};
use lazymkv_core::robot::parse;
use lazymkv_core::recording::files::{read_jsonl, rip_label, MkvmergeIdent};
use lazymkv_core::recording::{read_text, text_mode, Encoding, Errors, Tree};
use lazymkv_core::reconcile::RIP_LOG_NAMES;
use lazymkv_core::review_doc::sample_lines;
use lazymkv_core::subtitle::text::ocr_records;
use lazymkv_core::verify::{by_scan_stream, selected_streams, subs_index, subs_streams, verify_rip};

use crate::cmd::decide::{read_unread, rec_folder};
use crate::evidence::{read_scan_in, Ctx, Recording};
use crate::{usage_error, Args};

const NAME: &str = "sheet";
const USAGE: &str = "sheet <job folder> [--text DIR] [--held] [--all]";

/// What the command read for the document (`DocInputs`' maps that it fills).
#[derive(Default)]
struct Read {
    lines: BTreeMap<(String, u32), Vec<String>>,
    unread: BTreeMap<(String, u32), String>,
}

/// `lazymkv-dev sheet`: the arguments, the job folder, the sheet on `out`, the exit code. An error line goes to `err`
/// only. Def: P3-T13. Use: the wording trial.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &["--text", "--held", "--all"]);
    let text = match parsed.flag("--text") {
        Ok(text) => text,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let held = parsed.switch("--held");
    let all = parsed.switch("--all");
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let [job] = words.as_slice() else {
        let message = if words.is_empty() {
            "the following arguments are required: job folder".to_string()
        } else {
            format!("unrecognized arguments: {}", words[1..].join(" "))
        };
        return usage_error(err, NAME, USAGE, &message);
    };
    match sheet(ctx, job, text.as_deref(), held, all) {
        Ok(sheet) => {
            let _ = write!(out, "{sheet}");
            0
        }
        Err(message) => {
            let _ = writeln!(err, "lazymkv-dev {NAME}: {message}");
            2
        }
    }
}

/// The rendered sheet of the job folder `job`; Err is the message for standard error.
fn sheet(ctx: &Ctx, job: &str, text: Option<&str>, held: bool, all: bool) -> Result<String, String> {
    let job_dir = ctx.abs(job);
    let state = job_dir.join("state.json");
    let shown = |error: String| format!("{}: {error}", state.display());
    let raw = fs::read_to_string(&state).map_err(|error| shown(error.to_string()))?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|error| shown(error.to_string()))?;
    check_format(&value, JOB_FORMAT).map_err(shown)?;
    let mut record: JobRecord = serde_json::from_value(value).map_err(|error| shown(error.to_string()))?;
    if held {
        // In memory only: a filed job's sheet as it was when it was held.
        record.state = State::NeedsReview { held: Held::DispositionGate };
    }
    let scanned = match read_scan(&job_dir) {
        Ok(scanned) => scanned,
        Err(error) => {
            // A job with nothing to review may have no scan: `build`'s refusal comes first, as the engine's does.
            let empty = disc(&parse(""))?;
            let none = Read::default();
            // Any other failure of that build is the empty scan's, so the scan's own error is the one to give.
            return match review_doc::build(&inputs(&record, &empty, &none, all)) {
                Err(refusal) if refusal.ends_with("there is nothing to review") => Err(refusal),
                _ => Err(error),
            };
        }
    };
    let mut read = Read::default();
    if let Some(text) = text {
        read_lines(ctx, &job_dir, &record, &scanned, &ctx.abs(text), &mut read);
    }
    let doc = review_doc::build(&inputs(&record, &scanned, &read, all))?;
    Ok(review_doc::render(&doc))
}

fn inputs<'a>(record: &'a JobRecord, scanned: &'a Disc, read: &'a Read, all: bool) -> DocInputs<'a> {
    // No share is looked at: no staged file, no colliding file and no empty spot.
    static NONE_STAGED: BTreeMap<String, String> = BTreeMap::new();
    static NONE_COLLIDING: BTreeMap<String, String> = BTreeMap::new();
    static NONE_EMPTY: BTreeSet<String> = BTreeSet::new();
    DocInputs {
        record,
        disc: scanned,
        staged: &NONE_STAGED,
        lines: &read.lines,
        unread: &read.unread,
        collisions: &NONE_COLLIDING,
        nothing_at: &NONE_EMPTY,
        all,
    }
}

/// The job folder's `scan.txt`, typed; Err `<path>: <error>`.
fn read_scan(job_dir: &Path) -> Result<Disc, String> {
    let path = job_dir.join("scan.txt");
    let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    disc(&parse(&String::from_utf8_lossy(&bytes))).map_err(|error| format!("{}: {error}", path.display()))
}

/// The lines of every subtitle stream the decisions list, as the engine's worker reads them: `subtitle_lines` over
/// each title's label folder, the title's length from the scan, and the streams of its `subs-ocr/summary.json` that
/// were not read as `unread`. A title whose files cannot be read gives each of its subtitle streams `the subtitle
/// reading could not be read: <error>`, whose error names the file only.
fn read_lines(ctx: &Ctx, job_dir: &Path, record: &JobRecord, scanned: &Disc, text: &Path, read: &mut Read) {
    for title in &record.titles {
        let Some(decision) = &title.decision else { continue };
        let kept = decision.subtitles.kept.iter().map(|track| track.stream);
        let dropped = decision.subtitles.dropped.iter().map(|(stream, _)| *stream);
        let streams: Vec<u32> = kept.chain(dropped).collect();
        let seconds = scanned.title(title.scan_title).ok().and_then(|t| t.seconds).map_or(0.0, f64::from);
        let found = rec_folder(ctx, &job_dir.join(&title.key).display().to_string())
            .map_err(|refused| refused.line)
            .and_then(|(rec, label)| {
                let lines = subtitle_lines(&rec, &label, text, seconds)?;
                let by_n: BTreeMap<i64, String> = read_unread(&rec.tree, &rec.dir.join(&label))?.into_iter().collect();
                let skipped: Vec<(u32, String)> = scan_streams(&rec, &label)?
                    .into_iter()
                    .filter_map(|(n, stream)| by_n.get(&n).map(|why| (stream, why.clone())))
                    .collect();
                Ok((lines, skipped))
            });
        match found {
            Ok((lines, skipped)) => {
                for (stream, shown) in lines {
                    read.lines.insert((title.key.clone(), stream), shown);
                }
                for (stream, why) in skipped {
                    read.unread.insert((title.key.clone(), stream), why);
                }
            }
            Err(error) => {
                for stream in streams {
                    let why = format!("the subtitle reading could not be read: {error}");
                    read.unread.entry((title.key.clone(), stream)).or_insert(why);
                }
            }
        }
    }
}

/// Each `sub-<n>.ocr.jsonl` of `text`'s lines (phase 3 design, part 6.2; ruling T32-R5): every record's time (`pts`,
/// else `t`) and text lines (`lines` when it is a list, else `text`), in time order, through `review_doc::sample_lines`
/// at `seconds`, the scan title's length, keyed by the scan stream that `n` is (`scan_streams`, as the decision's text
/// is mapped); a stream with a reading file but no event with text gives an empty list, and an `n` that maps to no scan
/// stream is left out. Err `<file>: <error>` for a reading file that cannot be read or parsed, and `scan_streams`'
/// error; no error holds subtitle text. Def: P3-T9. Use: P3-T13, P3-T14, P3-T16.
pub fn subtitle_lines(
    rec: &Recording,
    label: &str,
    text: &Path,
    seconds: f64,
) -> Result<BTreeMap<u32, Vec<String>>, String> {
    let streams = scan_streams(rec, label)?;
    let mut by_n: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for &n in streams.keys() {
        let file = text.join(format!("sub-{n}.ocr.jsonl"));
        if !file.is_file() {
            continue;
        }
        // The errors name the file and the reader's or parser's position only, never the text.
        let shown = |error: String| format!("{}: {error}", file.display());
        let bytes = fs::read(&file).map_err(|error| shown(error.to_string()))?;
        let read = String::from_utf8(bytes).map_err(|error| shown(error.to_string()))?;
        let records = read_jsonl(&read).map_err(shown)?;
        let mut events: Vec<(f64, Vec<String>)> = ocr_records(&records, &BTreeMap::new())
            .map_err(shown)?
            .into_iter()
            .map(|record| (record.t, record.lines))
            .collect();
        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        by_n.insert(n, sample_lines(&events, seconds));
    }
    Ok(by_scan_stream(&by_n, &streams))
}

/// The scan stream of each `n` of `<label>/subs/index.json` (phase 3 design, part 6.2): the label's title and
/// selection from its rip label (`rip_label`), `verify::verify_rip` of the scan title against `<label>/mkvmerge.json`
/// with the rip log, as decide's decision section verifies it, then `verify::subs_streams`. Empty when the label has no
/// `subs/index.json`; Err with the reader's sentence for a file that cannot be read. Def: P3-T9. Use: P3-T13, P3-T14,
/// P3-T16.
pub fn scan_streams(rec: &Recording, label: &str) -> Result<BTreeMap<i64, u32>, String> {
    let tree: &dyn Tree = &rec.tree;
    let label_dir = rec.dir.join(label);
    let subs = label_dir.join("subs");
    if !tree.is_file(&subs.join("index.json")) {
        return Ok(BTreeMap::new());
    }
    let rip = rip_label(tree, &rec.dir, label)?;
    let number = u32::try_from(rip.title).map_err(|_| format!("{label}: the summary names title {}", rip.title))?;
    let scan = read_scan_in(tree, &rec.dir.join("scan.txt"), Encoding::Utf8, Errors::Replace)?;
    let typed = disc(&scan)?;
    let scanned = typed.title(number)?;
    let selection = selected_streams(&scan, number, rip.selection)?;
    let ident_path = label_dir.join("mkvmerge.json");
    let ident: MkvmergeIdent = serde_json::from_str(&read_text(tree, &ident_path, Encoding::Utf8Sig, Errors::Strict)?)
        .map_err(|error| format!("{}: {error}", tree.display(&ident_path)))?;
    let rip_log = match RIP_LOG_NAMES.iter().map(|name| label_dir.join(name)).find(|path| tree.exists(path)) {
        Some(path) => Some(text_mode(&read_text(tree, &path, Encoding::Utf8, Errors::Replace)?)),
        None => None,
    };
    let report = verify_rip(scanned, &ident, &selection, rip_log.as_deref(), typed.is_dvd);
    Ok(subs_streams(&subs_index(tree, &subs)?, &report))
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::{json, Value};

    use crate::evidence::{resolve_recording, Ctx};
    use crate::testutil::{write, Scratch};

    /// A ten-minute title: video, English audio and two English PGS streams, 2 and 3.
    const SCAN: &str = r#"TINFO:0,8,0,"2"
TINFO:0,9,0,"0:10:00"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"Mpeg4"
SINFO:0,1,1,6202,"Audio"
SINFO:0,1,3,0,"eng"
SINFO:0,1,6,0,"DD"
SINFO:0,1,14,0,"2"
SINFO:0,2,1,6203,"Subtitles"
SINFO:0,2,3,0,"eng"
SINFO:0,2,6,0,"PGS"
SINFO:0,3,1,6203,"Subtitles"
SINFO:0,3,3,0,"eng"
SINFO:0,3,6,0,"PGS"
"#;

    /// recs/disc-s/t00 under `root`: scan.txt, an ok rip-summary.json naming title 0, mkvmerge.json with one track
    /// per scan stream, and subs/index.json whose `n` 0 is file track 3 and `n` 1 file track 2, so each `n` is the
    /// other scan stream's. Returns the recording and the label.
    fn recording(root: &Path) -> (Recording, String) {
        let rec = root.join("recs/disc-s");
        write(&rec.join("scan.txt"), SCAN);
        let label = rec.join("t00");
        write(&label.join("rip-summary.json"), json!({"ok": true, "title": 0}).to_string());
        let ident = json!({
            "tracks": [
                {"id": 0, "type": "video", "codec": "AVC", "properties": {"language": "und"}},
                {"id": 1, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}},
                {"id": 2, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}},
                {"id": 3, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}}
            ],
            "chapters": [{"num_entries": 2}],
            "container": {"properties": {"duration": 600_000_000_000_i64}}
        });
        write(&label.join("mkvmerge.json"), ident.to_string());
        let index: Vec<Value> = [(0, 3), (1, 2)]
            .iter()
            .map(|(n, track)| json!({"n": n, "track_id": track, "language": "eng", "codec": "HDMV PGS"}))
            .collect();
        write(&label.join("subs/index.json"), Value::Array(index).to_string());
        recording_without_writing(root)
    }

    /// recs/disc-s under `root` opened as it is on disk, and its label t00.
    fn recording_without_writing(root: &Path) -> (Recording, String) {
        let ctx = Ctx::for_test(root);
        let rec = root.join("recs/disc-s");
        (resolve_recording(&ctx, None, &rec.display().to_string()).unwrap(), "t00".to_string())
    }

    /// One reading record per (time, text lines), as the OCR worker writes them, one per line.
    fn reading(records: &[(f64, &[&str])]) -> String {
        records.iter().map(|(t, lines)| format!("{}\n", json!({"frame": 0, "t": t, "pts": t, "lines": lines}))).collect()
    }

    #[test]
    fn two_streams_of_a_small_recording_give_their_lines_under_their_scan_stream_numbers() {
        let tmp = Scratch::new("sheet-lines");
        let (rec, label) = recording(&tmp.0);
        let text = tmp.0.join("text");
        // n 0 (scan stream 3): out of time order, and an event at 150 s that has no text.
        write(&text.join("sub-0.ocr.jsonl"), reading(&[(320.0, &["made-up line b"]), (100.0, &["made-up line a"]),
            (150.0, &[" "]), (160.0, &["made-up line c", "and its second half"]), (460.0, &["made-up line d"])]));
        // n 1 (scan stream 2): the `text` form of an older reading, with no `lines` list.
        write(&text.join("sub-1.ocr.jsonl"), format!("{}\n", json!({"frame": 0, "t": 300.0, "text": "made-up line e"})));
        let lines = subtitle_lines(&rec, &label, &text, 600.0).unwrap();
        let expected: BTreeMap<u32, Vec<String>> = BTreeMap::from([
            (3, vec!["made-up line c and its second half".to_string(), "made-up line b".to_string(),
                     "made-up line d".to_string()]),
            (2, vec!["made-up line e".to_string()]),
        ]);
        assert_eq!(lines, expected);
        assert_eq!(scan_streams(&rec, &label).unwrap(), BTreeMap::from([(0, 3), (1, 2)]));
    }

    #[test]
    fn a_reading_with_no_text_gives_an_empty_list_and_a_broken_one_an_error_naming_only_the_file() {
        let tmp = Scratch::new("sheet-broken");
        let (rec, label) = recording(&tmp.0);
        let text = tmp.0.join("text");
        write(&text.join("sub-0.ocr.jsonl"), reading(&[(100.0, &[])]));
        assert_eq!(subtitle_lines(&rec, &label, &text, 600.0).unwrap(), BTreeMap::from([(3, Vec::new())]));
        write(&text.join("sub-1.ocr.jsonl"), "{\"t\": 1.0, \"lines\": [\"made-up secret\"\n");
        let error = subtitle_lines(&rec, &label, &text, 600.0).unwrap_err();
        let file = text.join("sub-1.ocr.jsonl");
        assert!(error.starts_with(&format!("{}: ", file.display())), "{error}");
        assert!(!error.contains("made-up secret"), "{error}");
    }

    use std::collections::BTreeSet;

    use lazymkv_core::decision::{Confidences, Disposition, FileDecision};
    use lazymkv_core::disc::Disc;
    use lazymkv_core::job::{Held, State};
    use lazymkv_core::lang::wanted_languages;
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use lazymkv_core::lookup::{Identity, IdentityStep, Kind, RuntimeFacts};
    use lazymkv_core::naming::NamedPath;
    use lazymkv_core::plan::movie::TitleRule;
    use lazymkv_core::plan::{KindSource, Mode, Plan, PlanEvidence, PlannedRole, PlannedTitle};
    use lazymkv_core::record::{JobRecord, TitleRecord, JOB_FORMAT};
    use lazymkv_core::review::{Noted, ReviewReason};
    use lazymkv_core::review_doc::{build, render, DocInputs};
    use lazymkv_core::tracks::audio::{AudioDecision, AudioTrack, LangGroup};
    use lazymkv_core::tracks::layout::layout;
    use lazymkv_core::tracks::subtitle::{SubtitleCase, SubtitleDecision, SubtitleDrop, SubtitleTrack};
    use lazymkv_core::tracks::SubtitleRole;

    use crate::testutil::call;

    /// The fixture's title key, which is also the title's label folder under the job folder.
    const KEY: &str = "t00-00800";

    fn scanned(job: &Path) -> Disc {
        let text = std::fs::read_to_string(job.join("scan.txt")).unwrap();
        lazymkv_core::disc::disc(&lazymkv_core::robot::parse(&text)).unwrap()
    }

    fn kept_subtitle(stream: u32) -> SubtitleTrack {
        SubtitleTrack {
            stream,
            role: SubtitleRole::Regular,
            name: "English".to_string(),
            forced: false,
            hearing_impaired: false,
            default: false,
            events: Some(100),
            closed_captions: false,
            parent: None,
            decided_by: "alignment".to_string(),
        }
    }

    /// A decision keeping audio 1 and subtitles 2 and 3, the two subtitles ambiguous.
    fn decision(disc: &Disc) -> FileDecision {
        let scan = disc.title(0).unwrap();
        let stream = scan.streams.iter().find(|s| s.number == 1).unwrap();
        let audio = AudioDecision {
            kept: vec![AudioTrack {
                stream: 1,
                language: stream.language.clone(),
                group: LangGroup::English,
                codec: stream.codec.clone(),
                file_codec: stream.file_codec.clone(),
                channels: 2,
                tier: None,
                object: stream.codec.object,
                sample_size: None,
                sample_rate: None,
                bitrate_kbps: None,
                first_of_language: true,
                name: "Dolby Digital 2.0".to_string(),
                default: true,
                original: true,
            }],
            dropped: vec![],
            reference: Some(1),
            verdict_reference: None,
            notes: vec![],
            reviews: vec![],
            confidence: 0.95,
            description_alignment: None,
        };
        let subtitles = SubtitleDecision {
            kept: vec![kept_subtitle(2), kept_subtitle(3)],
            dropped: Vec::<(u32, SubtitleDrop)>::new(),
            set_aside: vec![],
            full: vec![2, 3],
            groups: vec![vec![2], vec![3]],
            dialogue_group: Some(0),
            dialogue_pick: None,
            pairs: vec![],
            case: SubtitleCase::Ambiguous,
            confidence: 0.60,
            notes: Vec::<Noted>::new(),
            reviews: vec![ReviewReason::SubtitleAmbiguous],
        };
        let layout = layout(scan, &audio, &subtitles);
        FileDecision {
            title_key: KEY.to_string(),
            audio,
            subtitles,
            layout,
            resolution: Some("1080p".to_string()),
            edition: None,
            path: Some(NamedPath {
                root: "M:/Movies".to_string(),
                components: vec!["Film (2000)".to_string(), "Film (2000).mkv".to_string()],
                path: "M:/Movies/Film (2000)/Film (2000).mkv".to_string(),
            }),
            confidences: Confidences { identity: 0.90, title: 0.98, track: 0.60 },
            reviews: vec![ReviewReason::SubtitleAmbiguous],
            notes: vec![],
            read_errors: vec![],
            disposition: Disposition::NeedsReview,
        }
    }

    /// A job held at the disposition gate with the one title decided.
    fn record(disc: &Disc) -> JobRecord {
        let title = disc.title(0).unwrap();
        let plan = Plan {
            identity: Identity {
                step: IdentityStep::TmdbOne,
                confidence: 0.90,
                tmdb: Some(TmdbIdentity { kind: TmdbKind::Movie, id: 1 }),
                title: Some("Film".to_string()),
                year: Some(2000),
                original_language: Some("en".to_string()),
                review: None,
                plausible: vec![],
                runtime_check: Some(true),
                lookup_pending: false,
                evidence: vec![],
            },
            kind: Kind::Movie,
            kind_source: KindSource::Discdb,
            titles: vec![PlannedTitle {
                key: KEY.to_string(),
                title: 0,
                role: PlannedRole::Feature { edition: None, theatrical: true },
                seconds: title.seconds,
                size: title.size,
                source_file: title.source_file.clone(),
                suggested: false,
                remembered: false,
            }],
            mode: Mode::Direct,
            identity_confidence: 0.90,
            title_confidence: 0.98,
            reviews: vec![],
            notes: vec![],
            wanted: wanted_languages(Some("en")),
            session: None,
            evidence: PlanEvidence {
                name: None,
                candidates: vec![0],
                groups: vec![],
                rivals: vec![],
                title_rule: Some(TitleRule::Database),
                labels: vec![],
                discdb_matches: vec![],
                runtime: RuntimeFacts::default(),
                sentences: vec![],
            },
        };
        JobRecord {
            format: JOB_FORMAT.to_string(),
            id: "job-a".to_string(),
            fingerprint: "ABC".to_string(),
            disc_id: None,
            drive: "drive-0".to_string(),
            created_unix: 1_800_000_000,
            updated_unix: 1_800_000_900,
            state: State::NeedsReview { held: Held::DispositionGate },
            message: None,
            rip_again: false,
            profile: "job.mmcp.xml".to_string(),
            plan: Some(plan),
            titles: vec![TitleRecord {
                key: KEY.to_string(),
                scan_title: 0,
                rip_index: Some(0),
                attempts: 1,
                verified: true,
                verify_failures: 0,
                read_errors: vec![],
                rip_reviews: vec![],
                decision: Some(decision(disc)),
                filed: None,
                rip_started_ms: None,
            }],
            filed_unix: None,
            collisions: vec![],
            notes: vec![],
            tuning_copy: None,
        }
    }

    /// The test's job folder: the small recording of the tests above, its title's label folder named `KEY`, and a
    /// state.json holding `state` over the fixture record. Returns the folder and the record as written.
    fn job_folder(root: &Path, state: State) -> (std::path::PathBuf, JobRecord) {
        let (_, label) = recording(root);
        let job = root.join("recs/disc-s");
        let disc = scanned(&job);
        let mut held = record(&disc);
        held.state = state;
        if held.state == State::Filed {
            held.filed_unix = Some(1_800_001_000);
        }
        std::fs::rename(job.join(&label), job.join(KEY)).unwrap();
        write(&job.join("state.json"), serde_json::to_string(&held).unwrap());
        (job, held)
    }

    /// The sheet of `record` over the job folder's scan, as the command builds it with no lines, no staged file and no
    /// library.
    fn expected_sheet(job: &Path, record: &JobRecord, all: bool) -> String {
        let text = std::fs::read_to_string(job.join("scan.txt")).unwrap();
        let disc = lazymkv_core::disc::disc(&lazymkv_core::robot::parse(&text)).unwrap();
        let (staged, lines, unread, collisions) = Default::default();
        let doc = build(&DocInputs {
            record,
            disc: &disc,
            staged: &staged,
            lines: &lines,
            unread: &unread,
            collisions: &collisions,
            nothing_at: &BTreeSet::new(),
            all,
        })
        .unwrap();
        render(&doc)
    }

    fn run_sheet(root: &Path, args: &[&str]) -> (i32, String, String) {
        call(run, &Ctx::for_test(root), args)
    }

    #[test]
    fn a_held_job_folder_prints_its_sheet() {
        let tmp = Scratch::new("sheet-held");
        let (job, held) = job_folder(&tmp.0, State::NeedsReview { held: Held::DispositionGate });
        let (code, out, err) = run_sheet(&tmp.0, &[&job.display().to_string()]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, expected_sheet(&job, &held, false));
        assert!(out.contains("two full English tracks that do not match each other"), "{out}");
    }

    #[test]
    fn held_shows_a_filed_job_as_it_was_held() {
        let tmp = Scratch::new("sheet-filed");
        let (job, filed) = job_folder(&tmp.0, State::Filed);
        let folder = job.display().to_string();
        // A filed job is shown with every title's points; held shows it as the gate held it, with its one point.
        let (code, out, _) = run_sheet(&tmp.0, &[&folder]);
        assert_eq!((code, out.as_str()), (0, expected_sheet(&job, &filed, false).as_str()));
        let (code, held_out, err) = run_sheet(&tmp.0, &[&folder, "--held"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let mut as_held = filed.clone();
        as_held.state = State::NeedsReview { held: Held::DispositionGate };
        assert_eq!(held_out, expected_sheet(&job, &as_held, false));
        assert_ne!(held_out, out);
        assert!(held_out.starts_with("Held since "), "{held_out}");
        // Nothing was written: the state file still says Filed.
        let kept: JobRecord = serde_json::from_str(&std::fs::read_to_string(job.join("state.json")).unwrap()).unwrap();
        assert_eq!(kept.state, State::Filed);
    }

    #[test]
    fn text_adds_the_lines() {
        let tmp = Scratch::new("sheet-text");
        let (job, _) = job_folder(&tmp.0, State::NeedsReview { held: Held::DispositionGate });
        let text = tmp.0.join("text");
        // n 0 is scan stream 3 and n 1 scan stream 2 (the recording's index).
        write(&text.join("sub-0.ocr.jsonl"), reading(&[(100.0, &["made-up line a"]), (300.0, &["made-up line b"])]));
        write(&text.join("sub-1.ocr.jsonl"), reading(&[(200.0, &["made-up line c"])]));
        let (code, out, err) = run_sheet(&tmp.0, &[&job.display().to_string(), "--text", &text.display().to_string()]);
        assert_eq!((code, err.as_str()), (0, ""));
        let plain = run_sheet(&tmp.0, &[&job.display().to_string()]).1;
        assert!(!plain.contains("made-up line"), "{plain}");
        let row = |stream: u32| out.lines().find(|line| line.trim_start().starts_with(&format!("track {stream} "))).unwrap_or("");
        assert!(row(2).contains("made-up line c") && !row(2).contains("line a"), "{out}");
        assert!(row(3).contains("made-up line a") && row(3).contains("made-up line b"), "{out}");
    }

    #[test]
    fn a_job_with_nothing_to_review_exits_2_naming_it() {
        let tmp = Scratch::new("sheet-nothing");
        // A job that is neither held nor filed has nothing to review.
        let (job, _) = job_folder(&tmp.0, State::Ripping);
        let (code, out, err) = run_sheet(&tmp.0, &[&job.display().to_string()]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev sheet: job job-a is Ripping: there is nothing to review\n");
        // With no scan.txt the same refusal comes first.
        std::fs::remove_file(job.join("scan.txt")).unwrap();
        let (code, out, err) = run_sheet(&tmp.0, &[&job.display().to_string()]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("there is nothing to review"), "{err}");
    }

    #[test]
    fn an_argument_error_exits_2() {
        let tmp = Scratch::new("sheet-args");
        let (job, _) = job_folder(&tmp.0, State::NeedsReview { held: Held::DispositionGate });
        let folder = job.display().to_string();
        for args in [vec![], vec![folder.as_str(), "extra"], vec![folder.as_str(), "--nonsense"], vec![folder.as_str(), "--text"]] {
            let (code, out, err) = run_sheet(&tmp.0, &args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}: {err}");
            assert!(err.contains("lazymkv-dev sheet: error: "), "{args:?}: {err}");
        }
    }

    #[test]
    fn a_held_job_without_a_scan_exits_2_naming_the_scan() {
        let tmp = Scratch::new("sheet-noscan");
        let (job, _) = job_folder(&tmp.0, State::NeedsReview { held: Held::DispositionGate });
        std::fs::remove_file(job.join("scan.txt")).unwrap();
        let (code, out, err) = run_sheet(&tmp.0, &[&job.display().to_string()]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("scan.txt"), "{err}");
        assert!(!err.contains("nothing to review"), "{err}");
    }

    #[test]
    fn text_that_cannot_be_read_says_so_on_the_sheet() {
        let tmp = Scratch::new("sheet-badtext");
        let (job, _) = job_folder(&tmp.0, State::NeedsReview { held: Held::DispositionGate });
        let text = tmp.0.join("text");
        write(&text.join("sub-0.ocr.jsonl"), reading(&[(100.0, &["made-up line a"])]));
        write(&text.join("sub-1.ocr.jsonl"), "{\"t\": 1.0, \"lines\": [\"made-up secret\"\n");
        let (code, out, err) = run_sheet(&tmp.0, &[&job.display().to_string(), "--text", &text.display().to_string()]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(out.contains("the subtitle reading could not be read"), "{out}");
        assert!(out.contains("sub-1.ocr.jsonl"), "{out}");
        assert!(!out.contains("made-up secret"), "{out}");
    }

    #[test]
    fn a_stream_the_summary_records_as_not_read_says_why_on_the_sheet() {
        let tmp = Scratch::new("sheet-unread");
        let (job, _) = job_folder(&tmp.0, State::NeedsReview { held: Held::DispositionGate });
        let text = tmp.0.join("text");
        write(&text.join("sub-1.ocr.jsonl"), reading(&[(200.0, &["made-up line c"])]));
        let summary = json!({"streams": [{"n": 0, "read": false, "why": "made-up reason it was cut off"}]});
        write(&job.join(KEY).join("subs-ocr/summary.json"), summary.to_string());
        let (code, out, err) = run_sheet(&tmp.0, &[&job.display().to_string(), "--text", &text.display().to_string()]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(out.contains("made-up reason it was cut off"), "{out}");
        assert!(out.contains("made-up line c"), "{out}");
    }

    #[test]
    fn a_folder_without_a_state_file_exits_2_naming_it() {
        let tmp = Scratch::new("sheet-nostate");
        std::fs::create_dir_all(tmp.0.join("job")).unwrap();
        let (code, out, err) = run_sheet(&tmp.0, &["job"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("state.json"), "{err}");
    }

    #[test]
    fn a_label_without_a_subtitle_index_maps_nothing() {
        let tmp = Scratch::new("sheet-no-index");
        recording(&tmp.0);
        std::fs::remove_file(tmp.0.join("recs/disc-s/t00/subs/index.json")).unwrap();
        // The recording's tree is listed when it is opened, so it is opened again without the index.
        let (rec, label) = recording_without_writing(&tmp.0);
        assert_eq!(scan_streams(&rec, &label).unwrap(), BTreeMap::new());
        let text = tmp.0.join("text");
        write(&text.join("sub-0.ocr.jsonl"), reading(&[(200.0, &["made-up line"])]));
        assert_eq!(subtitle_lines(&rec, &label, &text, 600.0).unwrap(), BTreeMap::new());
    }
}
