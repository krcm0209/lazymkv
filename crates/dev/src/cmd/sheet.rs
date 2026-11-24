//! The review sheet's reader of subtitle lines (phase 3 design, part 6.2): the lines of a title's subtitle tracks that
//! step 10 read, taken at the three moments of ruling T32-R5 and keyed by scan stream. The text is held in memory only:
//! nothing here writes it, logs it or puts it in an error. A later task adds the `sheet` command to this file.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use lazymkv_core::disc::disc;
use lazymkv_core::recording::files::{read_jsonl, rip_label, MkvmergeIdent};
use lazymkv_core::recording::{read_text, text_mode, Encoding, Errors, Tree};
use lazymkv_core::reconcile::RIP_LOG_NAMES;
use lazymkv_core::review_doc::sample_lines;
use lazymkv_core::subtitle::text::ocr_records;
use lazymkv_core::verify::{by_scan_stream, selected_streams, subs_index, subs_streams, verify_rip};

use crate::evidence::{read_scan_in, Recording};

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
