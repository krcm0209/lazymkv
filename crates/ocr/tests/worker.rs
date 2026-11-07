//! The OCR worker over made-up evidence files (design part 10.7; notes F1). Every bitmap is a block shape with no
//! glyph and no font (RP21); no test asserts recognised text. Runs on ubuntu on CI and on the mini PC.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lazymkv_core::subtitle::bitmap::evidence::{read_run_json, whole_records};
use lazymkv_core::subtitle::bitmap::mkv::read_stream as read_mks;
use lazymkv_core::subtitle::bitmap::{CODEC_PGS, CODEC_VOBSUB};
use lazymkv_core::testsupport::mks::{
    idx_text, mks, pgs_display_set, vobsub_unit, MksBlock, MksEncoding, MksShape, MksTrack,
};
use lazymkv_ocr::stream::{main_with, USAGE};
use lazymkv_ocr::{engine_id, read_png_grey};
use serde_json::Value;

const VIDEO: (u32, u32) = (1920, 1080);
/// Palette entry 1: Y 235, Cr 128, Cb 128, alpha 255, which prepares to grey 20 (design part 10.5).
const OPAQUE: (u8, [u8; 4]) = (1, [235, 128, 128, 255]);
/// The same colour with alpha 0, which prepares to white: an event with no dark pixel.
const TRANSPARENT: (u8, [u8; 4]) = (1, [235, 128, 128, 0]);
type Objects = &'static [(u16, u32, u32, u32, u32, u8)];
/// One line: a 300 x 40 block at (800, 900), below the frame's middle row 540.
const ONE: Objects = &[(0, 800, 900, 300, 40, 1)];
/// Two lines, 20 empty rows apart.
const TWO: Objects = &[(0, 700, 860, 400, 40, 1), (1, 700, 920, 300, 40, 1)];
/// Ink above and below the middle row: trigger 4 (design part 10.6).
const TOP_AND_BOTTOM: Objects = &[(0, 800, 100, 300, 40, 1), (1, 800, 900, 300, 40, 1)];
/// The size of a PGS clear display set (spec 8.7); the worker never decodes it.
const CLEAR_SET: [u8; 30] = [0x16; 30];
const RECORD_KEYS: [&str; 13] =
    ["frame", "file", "t", "pts", "lines", "text", "size", "boxes", "method", "trigger", "engine", "decoder", "ms"];

/// A fresh, empty folder under the system temporary folder for one test.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lazymkv-ocr-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A subtitle track (type 17) stored with zlib, as mkvmerge stores PGS and VobSub by default (K1).
fn subtitle(number: u64, codec_id: &str, codec_private: Vec<u8>, encoding: MksEncoding) -> MksTrack {
    MksTrack {
        number,
        kind: 17,
        codec_id: codec_id.to_string(),
        codec_private,
        language: "eng".to_string(),
        encoding,
    }
}

fn set(objects: Objects) -> Vec<u8> {
    pgs_display_set(VIDEO, objects, &[OPAQUE])
}

fn pgs_with(blocks: &[MksBlock]) -> Vec<u8> {
    mks(&[subtitle(1, CODEC_PGS, Vec::new(), MksEncoding::Zlib)], blocks, &MksShape::default())
}

/// ONE at 1 s, a clear at 3 s, TWO at 4 s, a clear at 6 s, TOP_AND_BOTTOM at 7 s and ONE again at 9 s.
fn pgs_file() -> Vec<u8> {
    pgs_with(&[
        MksBlock::new(1, 1000, set(ONE)),
        MksBlock::new(1, 3000, CLEAR_SET.to_vec()),
        MksBlock::new(1, 4000, set(TWO)),
        MksBlock::new(1, 6000, CLEAR_SET.to_vec()),
        MksBlock::new(1, 7000, set(TOP_AND_BOTTOM)),
        MksBlock::new(1, 9000, set(ONE)),
    ])
}

/// The reader's (pts, size) of every block larger than `clear_max`: the events the worker must record.
fn listed_events(bytes: &[u8], n: usize, clear_max: i64) -> Vec<(f64, u64)> {
    let stream = read_mks(bytes, n).unwrap();
    stream.blocks.iter().filter(|b| b.size as i64 > clear_max).map(|b| (b.pts, b.size)).collect()
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// The built worker, started as the engine starts it: an empty environment but RTEN_NUM_THREADS=3 (V1, design 10.7).
fn run_binary(evidence: &Path, stream: usize, clear_max: i64, out: &Path, cwd: &Path) -> Output {
    let (stream, clear_max) = (stream.to_string(), clear_max.to_string());
    Command::new(env!("CARGO_BIN_EXE_lazymkv-ocr"))
        .args([path(evidence), "--stream", stream.as_str(), "--clear-max", clear_max.as_str(), "--out", path(out)])
        .current_dir(cwd)
        .env_clear()
        .env("RTEN_NUM_THREADS", "3")
        .output()
        .unwrap()
}

fn in_process(args: &[&str]) -> (i32, String) {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let mut err = Vec::new();
    let code = main_with(&args, &mut err);
    (code, String::from_utf8(err).unwrap())
}

fn run_in_process(evidence: &Path, stream: usize, clear_max: i64, out: &Path) -> (i32, String) {
    let (stream, clear_max) = (stream.to_string(), clear_max.to_string());
    in_process(&[path(evidence), "--stream", &stream, "--clear-max", &clear_max, "--out", path(out)])
}

fn records(out: &Path, n: usize) -> (String, Vec<Value>) {
    let text = fs::read_to_string(out.join(format!("sub-{n}.ocr.jsonl"))).unwrap();
    let parsed = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    (text, parsed)
}

/// The names in a folder, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn strs<'a>(records: &'a [Value], key: &str) -> Vec<&'a str> {
    records.iter().map(|r| r[key].as_str().unwrap()).collect()
}

#[test]
fn the_worker_reads_a_made_up_pgs_stream_into_records_run_json_and_crops() {
    let root = scratch("pgs");
    let (evidence, out, cwd) = (root.join("evidence.mks"), root.join("out"), root.join("cwd"));
    fs::write(&evidence, pgs_file()).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    let run = run_binary(&evidence, 0, 64, &out, &cwd);
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(run.stdout.is_empty(), "the worker prints nothing on standard output");

    let listed = listed_events(&pgs_file(), 0, 64);
    assert_eq!(listed.iter().map(|e| e.0).collect::<Vec<_>>(), [1.0, 4.0, 7.0, 9.0]);
    let (text, records) = records(&out, 0);
    assert_eq!(whole_records(&text), listed, "one record per event, in pts order, with the reader's sizes");
    for (i, record) in records.iter().enumerate() {
        let keys: BTreeSet<&str> = record.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, RECORD_KEYS.into_iter().collect::<BTreeSet<_>>());
        assert_eq!(record["frame"], i);
        assert_eq!(record["t"], record["pts"]);
        assert!(record["lines"].is_array());
    }
    assert_eq!(strs(&records, "method"), ["rows", "rows", "detector", "rows"]);
    let triggers: Vec<Value> = records.iter().map(|r| r["trigger"].clone()).collect();
    assert_eq!(triggers, [Value::Null, Value::Null, Value::from(4), Value::Null]);
    let boxes: Vec<usize> = records.iter().map(|r| r["boxes"].as_array().unwrap().len()).collect();
    assert_eq!((boxes[0], boxes[1], boxes[3]), (1, 2, 1));
    assert_eq!(strs(&records, "file"), ["f-000001000.png", "f-000004000.png", "f-000007000.png", "f-000001000.png"]);

    let crops = names(&out.join("s0"));
    assert_eq!(crops, ["f-000001000.png", "f-000004000.png", "f-000007000.png"]);
    for name in &crops {
        read_png_grey(&fs::read(out.join("s0").join(name)).unwrap()).unwrap();
    }
    let one = read_png_grey(&fs::read(out.join("s0").join("f-000001000.png")).unwrap()).unwrap();
    assert_eq!((one.width, one.height), (332, 72), "the 300 x 40 block grown by 16 on each side");

    let totals = read_run_json(&fs::read_to_string(out.join("sub-0.run.json")).unwrap()).unwrap();
    assert_eq!((totals.events, totals.distinct, totals.rows, totals.detector), (4, 3, 3, [0, 0, 0, 1]));
    assert_eq!(totals.engine, engine_id());
    assert!(totals.wall_ms > 0);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn repeated_bitmaps_share_one_crop_and_one_reading() {
    let root = scratch("repeat");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    let blocks = [MksBlock::new(1, 1000, set(ONE)), MksBlock::new(1, 2000, set(ONE)), MksBlock::new(1, 5000, set(ONE))];
    fs::write(&evidence, pgs_with(&blocks)).unwrap();
    assert_eq!(run_in_process(&evidence, 0, 64, &out), (0, String::new()));
    let (_, records) = records(&out, 0);
    assert_eq!(strs(&records, "file"), ["f-000001000.png"; 3]);
    for repeat in &records[1..] {
        assert_eq!(repeat["lines"], records[0]["lines"]);
        assert_eq!(repeat["boxes"], records[0]["boxes"]);
        assert_eq!(repeat["ms"]["recognize"], 0.0, "a repeat is not read again");
        assert_eq!(repeat["ms"]["detect"], Value::Null);
    }
    assert_eq!(names(&out.join("s0")), ["f-000001000.png"]);
    let totals = read_run_json(&fs::read_to_string(out.join("sub-0.run.json")).unwrap()).unwrap();
    assert_eq!((totals.events, totals.distinct, totals.rows), (3, 1, 3));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_made_up_vobsub_stream_reads_too() {
    let root = scratch("vobsub");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    let mut palette = [[0u8; 3]; 16];
    palette[1] = [255, 255, 255];
    let unit = |width| vobsub_unit((100, 380, width, 40), 1, [0, 1, 0, 0], [0, 15, 0, 0]);
    // A SubRip track first, as MakeMKV's DVD closed captions come (R6-phase0-14), so the VobSub track is stream 1.
    let tracks = [
        subtitle(1, "S_TEXT/UTF8", Vec::new(), MksEncoding::None),
        subtitle(2, CODEC_VOBSUB, idx_text(720, 480, &palette), MksEncoding::Zlib),
    ];
    let blocks = [
        MksBlock::new(1, 500, b"sentinel".to_vec()),
        MksBlock::new(2, 2000, unit(200)),
        MksBlock::new(2, 6000, unit(240)),
    ];
    let bytes = mks(&tracks, &blocks, &MksShape::default());
    fs::write(&evidence, &bytes).unwrap();
    let listed = listed_events(&bytes, 1, 64);
    assert_eq!(listed.len(), 2, "both units are larger than 64 bytes");
    assert_eq!(run_in_process(&evidence, 1, 64, &out), (0, String::new()));
    let (text, records) = records(&out, 1);
    assert_eq!(whole_records(&text), listed);
    assert_eq!(strs(&records, "file"), ["f-000002000.png", "f-000006000.png"]);
    assert_eq!(names(&out), ["s1", "sub-1.ocr.jsonl", "sub-1.run.json"]);
    let first = read_png_grey(&fs::read(out.join("s1").join("f-000002000.png")).unwrap()).unwrap();
    assert_eq!((first.width, first.height), (232, 72));
    let totals = read_run_json(&fs::read_to_string(out.join("sub-1.run.json")).unwrap()).unwrap();
    assert_eq!((totals.events, totals.distinct), (2, 2));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_argument_error_exits_2_with_the_usage() {
    let cases = [
        (vec![], "missing <evidence.mks>"),
        (vec!["e.mks", "--stream", "x", "--clear-max", "64", "--out", "o"], "--stream takes a whole number, not 'x'"),
        (vec!["e.mks", "--stream", "0", "--clear-max", "64"], "missing --out"),
        (vec!["e.mks", "--stream", "0", "--clear-max", "64", "--out", "o", "--fast", "1"], "unknown argument '--fast'"),
    ];
    for (args, first) in cases {
        assert_eq!(in_process(&args), (2, format!("{first}\n{USAGE}\n")), "{args:?}");
    }
}

#[test]
fn a_text_stream_is_refused_with_exit_3_naming_its_codec() {
    let root = scratch("text");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    let tracks = [subtitle(1, "S_TEXT/UTF8", Vec::new(), MksEncoding::None)];
    fs::write(&evidence, mks(&tracks, &[MksBlock::new(1, 500, b"sentinel".to_vec())], &MksShape::default())).unwrap();
    let refused = "subtitle stream 0 has codec S_TEXT/UTF8, which is not PGS or VobSub\n".to_string();
    assert_eq!(run_in_process(&evidence, 0, 64, &out), (3, refused));
    assert!(!out.exists(), "a refused stream writes nothing");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_laced_block_is_refused_with_exit_3() {
    let root = scratch("laced");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    let laced = MksBlock { laced: true, ..MksBlock::new(1, 1000, set(ONE)) };
    fs::write(&evidence, pgs_with(&[laced])).unwrap();
    assert_eq!(run_in_process(&evidence, 0, 64, &out), (3, "track 1 has a laced block at 1.000 s\n".to_string()));
    assert!(!out.exists());
    let _ = fs::remove_dir_all(&root);
}

/// Call 5: a content encoding other than zlib and header stripping refuses the stream with Task 3's sentence alone.
#[test]
fn another_content_encoding_is_refused_with_exit_3() {
    let root = scratch("encoding");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    let tracks = [subtitle(1, CODEC_PGS, Vec::new(), MksEncoding::Other(1))];
    fs::write(&evidence, mks(&tracks, &[MksBlock::new(1, 1000, set(ONE))], &MksShape::default())).unwrap();
    let refused = "content encoding 1 of track 1 is not zlib compression or header stripping\n".to_string();
    assert_eq!(run_in_process(&evidence, 0, 64, &out), (3, refused));
    assert!(!out.exists(), "a refused stream writes nothing");
    let _ = fs::remove_dir_all(&root);
}

/// Call 5: a display set the decoder refuses (Task 4) refuses the stream, naming the event and the decoder's sentence.
#[test]
fn a_display_set_the_decoder_refuses_exits_3() {
    let root = scratch("undecodable");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    // ONE's display set with its object segment's declared data length one more than it holds. A segment is its type
    // byte, a 2-byte length and its data; an object segment's data is its id (2), version (1), sequence flags (1),
    // data length (3, counting the 4 bytes of width and height), width (2), height (2) and the run-length data.
    let mut broken = set(ONE);
    let (mut at, mut rle) = (0, None);
    while at < broken.len() {
        let len = u16::from_be_bytes([broken[at + 1], broken[at + 2]]) as usize;
        if broken[at] == 0x15 {
            let declared = u32::from_be_bytes([0, broken[at + 7], broken[at + 8], broken[at + 9]]) + 1;
            broken[at + 7..at + 10].copy_from_slice(&declared.to_be_bytes()[1..]);
            rle = Some(len - 11);
            break;
        }
        at += 3 + len;
    }
    let rle = rle.expect("ONE's display set holds one object segment");
    fs::write(&evidence, pgs_with(&[MksBlock::new(1, 1000, broken)])).unwrap();
    let reason = format!("event 0 at 1.000 s does not decode: PGS object 0 holds {rle} bytes of run-length data, not {}\n", rle + 1);
    assert_eq!(run_in_process(&evidence, 0, 64, &out), (3, reason));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_stream_or_evidence_file_is_refused_with_exit_3() {
    let root = scratch("missing");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    fs::write(&evidence, pgs_file()).unwrap();
    assert_eq!(run_in_process(&evidence, 5, 64, &out), (3, "evidence.mks has no subtitle stream 5\n".to_string()));
    let (code, err) = run_in_process(&root.join("absent.mks"), 0, 64, &out);
    assert_eq!(code, 3);
    assert!(err.starts_with("the evidence file could not be read: ") && err.lines().count() == 1, "{err}");
    assert!(!err.contains("absent.mks"), "the reason names no path: {err}");
    assert!(!out.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_event_without_dark_pixels_gets_a_record_with_no_crop_and_no_reading() {
    let root = scratch("blank");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    let blank = pgs_display_set(VIDEO, ONE, &[TRANSPARENT]);
    fs::write(&evidence, pgs_with(&[MksBlock::new(1, 1000, set(ONE)), MksBlock::new(1, 2000, blank)])).unwrap();
    assert_eq!(run_in_process(&evidence, 0, 64, &out), (0, String::new()));
    let (_, records) = records(&out, 0);
    assert_eq!(records.len(), 2);
    let empty = &records[1];
    assert_eq!(
        (&empty["file"], &empty["lines"], &empty["boxes"], &empty["method"], &empty["trigger"]),
        (&Value::from(""), &Value::Array(vec![]), &Value::Array(vec![]), &Value::from("rows"), &Value::Null)
    );
    assert_eq!(empty["ms"]["recognize"], 0.0);
    assert_eq!(names(&out.join("s0")), ["f-000001000.png"]);
    let totals = read_run_json(&fs::read_to_string(out.join("sub-0.run.json")).unwrap()).unwrap();
    assert_eq!((totals.events, totals.distinct, totals.rows), (2, 1, 2));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_clear_max_argument_decides_which_packets_are_events() {
    let root = scratch("clear-max");
    let (evidence, out) = (root.join("evidence.mks"), root.join("out"));
    fs::write(&evidence, pgs_file()).unwrap();
    let smallest = listed_events(&pgs_file(), 0, 64).iter().map(|e| e.1).min().unwrap() as i64;
    let listed = listed_events(&pgs_file(), 0, smallest);
    // The two ONE sets (1 s and 9 s) are the smallest, one object each: at exactly --clear-max bytes they are clears.
    assert_eq!(listed.iter().map(|e| e.0).collect::<Vec<_>>(), [4.0, 7.0], "a packet of exactly --clear-max bytes is a clear");
    assert_eq!(run_in_process(&evidence, 0, smallest, &out), (0, String::new()));
    let (text, _) = records(&out, 0);
    assert_eq!(whole_records(&text), listed);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_worker_writes_nothing_outside_out() {
    let root = scratch("outside");
    let (evidence, out, cwd) = (root.join("evidence.mks"), root.join("out"), root.join("cwd"));
    fs::write(&evidence, pgs_file()).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    let run = run_binary(&evidence, 0, 64, &out, &cwd);
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(names(&root), ["cwd", "evidence.mks", "out"]);
    assert!(names(&cwd).is_empty(), "nothing in the working folder");
    assert_eq!(names(&out), ["s0", "sub-0.ocr.jsonl", "sub-0.run.json"]);
    assert_eq!(fs::read(&evidence).unwrap(), pgs_file(), "the evidence file is only read");
    let _ = fs::remove_dir_all(&root);
}
