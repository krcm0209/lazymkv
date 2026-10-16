//! `golden check` and `golden write` over made-up evidence clones (no disc, no real subtitle text: the OCR lines below
//! are invented ASCII) and over the evidence clone at corpus/owner once it holds the goldens (port spec section 5).
//! The first test is Review Focus item 5's golden-check half: a fresh clone, without level series and OCR text,
//! reports every comparison that needs them as skipped with its reason and exits 3 when nothing could run.

use std::fs;
use std::io::Write;
use std::path::Path;

use lazymkv_core::audio::dataset::{harness, HarnessInput, Overrides, BASELINE_FILE};
use lazymkv_core::audio::labels::exp13_requirements;
use lazymkv_core::testsupport::{commentary, level_file_text, programme, Scratch};
use lazymkv_dev::cmd::golden::{check, summary, write, Comparison, Outcome};
use lazymkv_dev::evidence::{default_spec, ocr_text_dir, repo_root, resolve_evidence, skip, Ctx, DiskTree, EVIDENCE_SHOWN};
use lazymkv_dev::pyjson::write_snapshot;
use lazymkv_dev::runner::{FakeRunner, Output, Runner};

/// A folder under the system's temporary folder, removed when dropped (the shared scratch folder).
type TempDir = Scratch;

/// Writes `bytes` to `root`/`rel`, making its folders first.
fn put(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    lazymkv_core::testsupport::write(&root.join(rel), bytes);
}

/// A test context run from the repository root (so --spec's default is the design spec), clock fixed, no ffmpeg.
fn ctx() -> Ctx {
    Ctx::for_test(&repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root"))
}

/// A Runner whose every program prints `version` as its first line.
fn answering(version: &'static str) -> Box<dyn Runner> {
    Box::new(FakeRunner::new(Box::new(move |_program: &str, _args: &[String], _cwd: Option<&Path>| -> std::io::Result<Output> {
        Ok(Output { status: 0, stdout: format!("{version}\nbuilt with made-up options\n"), stderr: String::new() })
    })))
}

/// A made-up Blu-ray scan: title 0, 7 minutes, video, two English AC-3 streams (6 and 2 channels), English PGS.
const SCAN: &str = concat!(
    "CINFO:1,6209,\"Blu-ray disc\"\n",
    "CINFO:2,0,\"DISC A\"\n",
    "TINFO:0,8,0,\"0\"\n",
    "TINFO:0,9,0,\"0:07:00\"\n",
    "TINFO:0,16,0,\"00001.mpls\"\n",
    "TINFO:0,26,0,\"1\"\n",
    "SINFO:0,0,1,6201,\"Video\"\n",
    "SINFO:0,0,6,0,\"V_MPEG4/ISO/AVC\"\n",
    "SINFO:0,1,1,6202,\"Audio\"\n",
    "SINFO:0,1,3,0,\"eng\"\n",
    "SINFO:0,1,6,0,\"A_AC3\"\n",
    "SINFO:0,1,14,0,\"6\"\n",
    "SINFO:0,2,1,6202,\"Audio\"\n",
    "SINFO:0,2,3,0,\"eng\"\n",
    "SINFO:0,2,6,0,\"A_AC3\"\n",
    "SINFO:0,2,14,0,\"2\"\n",
    "SINFO:0,3,1,6203,\"Subtitles\"\n",
    "SINFO:0,3,3,0,\"eng\"\n",
    "SINFO:0,3,6,0,\"S_HDMV/PGS\"\n",
);
const RIP_SUMMARY: &str = r#"{"ok": true, "result": "ok", "slug": "disc-a", "label": "all-tracks", "title": 0,
 "profile": "C:\\lazymkv\\phase0\\profiles\\all-tracks.mmcp.xml", "bytes": 1}"#;
const MKVMERGE: &str = r#"{"container": {"properties": {"duration": 420000000000}}, "chapters": [],
 "tracks": [
  {"id": 0, "type": "video", "codec": "AVC/H.264/MPEG-4p10", "properties": {"language": "und"}},
  {"id": 1, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 6}},
  {"id": 2, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}},
  {"id": 3, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}}]}"#;
const LEVELS: &str = "disc-a/all-tracks/audio-levels-20ms-1-2";
const STREAMS: &str = r#"{"streams": [
 {"index": 0, "codec_type": "video", "codec_name": "h264"},
 {"index": 1, "codec_type": "audio", "codec_name": "ac3", "channels": 6, "channel_layout": "5.1(side)", "tags": {"language": "eng"}},
 {"index": 2, "codec_type": "audio", "codec_name": "ac3", "channels": 2, "channel_layout": "stereo", "tags": {"language": "eng"}},
 {"index": 3, "codec_type": "subtitle", "codec_name": "hdmv_pgs_subtitle", "tags": {"language": "eng"}}]}"#;
const MANIFEST: &str = concat!(
    "# lazymkv audio-levels v2 (experiment 13)\n",
    "file=disc-a.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n",
    "main=1 codec=ac3 profile=- channels=6 layout=5.1(side) language=eng fc=yes frames=21000 ",
    "md5=00000000000000000000000000000001\n",
    "cand=2 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=21000 ",
    "md5=00000000000000000000000000000002\n",
);
const SUBS_INDEX: &str = r#"[{"n": 0, "track_id": 3, "language": "eng", "codec": "HDMV PGS", "track_name": null,
 "forced": false, "default": true}]"#;
const PROXY_GOLDEN: &str = r#"{"slug": "disc-a", "label": "all-tracks", "folder": "audio-levels-20ms-1-2", "main": 1,
 "cands": [2], "ffmpeg_version": "ffmpeg version 0.0-test", "proxy_sha1": "0000000000000000000000000000000000000000"}"#;
/// What the Python wrote for a level folder git keeps without its series (audio_dataset.measure).
const SERIES_ABSENT: &str = "disc-a/all-tracks/audio-levels-20ms-1-2: its level files are not here (git keeps only \
                             streams.json): pull the recording or copy the folder from the dataset share";
const NO_TEXT: &str = "no OCR text: disc-a/all-tracks/subs-ocr (give --text DIR or LAZYMKV_OCR_TEXT)";

/// 30 displayed packets (2000 bytes, every 2 s from 1 s) and 30 clearing packets (30 bytes) between them.
fn packets() -> String {
    let rows: Vec<String> = (0..30)
        .flat_map(|k| {
            [
                format!(r#"{{"pts_time": "{}.000000", "duration_time": "1.500000", "size": "2000"}}"#, 1 + 2 * k),
                format!(r#"{{"pts_time": "{}.500000", "duration_time": "N/A", "size": "30"}}"#, 2 + 2 * k),
            ]
        })
        .collect();
    format!("{{\"packets\": [{}]}}", rows.join(", "))
}

/// One OCR frame per displayed packet, one line each.
fn frames() -> String {
    let rows: Vec<String> = (0..30).map(|k| format!(r#"{{"frame": {k}, "lines": 1}}"#)).collect();
    format!("[{}]", rows.join(", "))
}

/// Invented OCR text: one made-up ASCII line per frame.
fn ocr_text() -> String {
    (0..30)
        .map(|k| format!("{{\"frame\": {k}, \"pts\": {}.0, \"lines\": [\"Made up line number {k} for the golden test.\"]}}\n", 1 + 2 * k))
        .collect()
}

/// The made-up recording disc-a with one rip label, all-tracks: scan, rip summary, mkvmerge.json, one level folder
/// (with its series only when `series`), and a subs folder with its OCR frame counts.
fn recording(root: &Path, series: bool) {
    put(root, "disc-a/scan.txt", SCAN);
    put(root, "disc-a/all-tracks/rip-summary.json", RIP_SUMMARY);
    put(root, "disc-a/all-tracks/mkvmerge.json", MKVMERGE);
    put(root, &format!("{LEVELS}/streams.json"), STREAMS);
    if series {
        let (mix, centre) = programme(420, 1);
        put(root, &format!("{LEVELS}/manifest.txt"), MANIFEST);
        put(root, &format!("{LEVELS}/main-mix.txt"), level_file_text(&mix));
        put(root, &format!("{LEVELS}/main-fc.txt"), level_file_text(&centre));
        put(root, &format!("{LEVELS}/cand-2.txt"), level_file_text(&commentary(&mix, 7)));
    }
    put(root, "disc-a/all-tracks/subs/index.json", SUBS_INDEX);
    put(root, "disc-a/all-tracks/subs/sub-0.packets.json", packets());
    put(root, "disc-a/all-tracks/subs-ocr/sub-0.frames.json", frames());
}

fn skipped(name: &str, reason: &str) -> Comparison {
    Comparison { name: name.to_string(), outcome: Outcome::Skipped(reason.to_string()) }
}

fn not_covered(name: &str) -> Comparison {
    skipped(name, &format!("not covered: {name} is not in the goldens"))
}

fn passed(name: &str) -> Comparison {
    Comparison { name: name.to_string(), outcome: Outcome::Pass }
}

#[test]
fn a_fresh_clone_reports_every_series_and_text_comparison_as_skipped_and_exits_3() {
    let dir = TempDir::new("fresh");
    let root = dir.0.as_path();
    recording(root, false);
    put(root, "disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.json", "{}\n");
    put(root, "disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.txt", "x\n");
    put(root, "disc-a/golden/all-tracks/subs.json", "{}\n");
    put(root, "disc-a/golden/all-tracks/subs.txt", "x\n");
    put(root, "disc-a/golden/proxy-audio-levels-20ms-1-2/golden.json", PROXY_GOLDEN);
    put(root, "golden/retune-report.txt", "x\n");
    put(root, BASELINE_FILE, "{}\n");
    let ctx = ctx();
    let comparisons = check(&ctx, root, None);
    assert_eq!(
        comparisons,
        vec![
            skipped("audio-dataset.json", SERIES_ABSENT),
            not_covered("disc-a/golden/all-tracks/align.json"),
            skipped("disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.json", SERIES_ABSENT),
            skipped("disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.txt", SERIES_ABSENT),
            not_covered("disc-a/golden/all-tracks/reconcile.json"),
            skipped("disc-a/golden/all-tracks/subs.json", NO_TEXT),
            skipped("disc-a/golden/all-tracks/subs.txt", NO_TEXT),
            not_covered("disc-a/golden/labels.json"),
            skipped("disc-a/golden/proxy-audio-levels-20ms-1-2", "ffmpeg is not on PATH"),
            not_covered("disc-a/golden/scan.json"),
            not_covered("disc-a/golden/titles.txt"),
            skipped("golden/retune-report.txt", SERIES_ABSENT),
        ]
    );
    assert_eq!(summary(&comparisons), ("golden check: 0 pass, 0 fail, 12 skipped".to_string(), 3));

    // What needs neither the series nor the text still runs on such a clone.
    let written = write(&ctx, root, None).unwrap();
    assert_eq!(
        written,
        vec![
            "wrote disc-a/golden/scan.json".to_string(),
            "wrote disc-a/golden/titles.txt".to_string(),
            "wrote disc-a/golden/labels.json".to_string(),
            "wrote disc-a/golden/all-tracks/reconcile.json".to_string(),
            format!("skipped disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.json: {SERIES_ABSENT}"),
            format!("skipped disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.txt: {SERIES_ABSENT}"),
            format!("skipped disc-a/golden/all-tracks/subs.json: {NO_TEXT}"),
            format!("skipped disc-a/golden/all-tracks/subs.txt: {NO_TEXT}"),
            "wrote disc-a/golden/all-tracks/align.json".to_string(),
            "skipped disc-a/golden/proxy-audio-levels-20ms-1-2: ffmpeg is not on PATH".to_string(),
            format!("skipped golden/retune-report.txt: {SERIES_ABSENT}"),
        ]
    );
    let comparisons = check(&ctx, root, None);
    assert_eq!(
        comparisons,
        vec![
            skipped("audio-dataset.json", SERIES_ABSENT),
            passed("disc-a/golden/all-tracks/align.json"),
            skipped("disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.json", SERIES_ABSENT),
            skipped("disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.txt", SERIES_ABSENT),
            passed("disc-a/golden/all-tracks/reconcile.json"),
            skipped("disc-a/golden/all-tracks/subs.json", NO_TEXT),
            skipped("disc-a/golden/all-tracks/subs.txt", NO_TEXT),
            passed("disc-a/golden/labels.json"),
            skipped("disc-a/golden/proxy-audio-levels-20ms-1-2", "ffmpeg is not on PATH"),
            passed("disc-a/golden/scan.json"),
            passed("disc-a/golden/titles.txt"),
            skipped("golden/retune-report.txt", SERIES_ABSENT),
        ]
    );
    assert_eq!(summary(&comparisons), ("golden check: 5 pass, 0 fail, 7 skipped".to_string(), 0));
}

#[test]
fn a_subs_run_with_an_english_stream_without_text_gives_no_subs_golden() {
    // Two English PGS streams, OCR text for s0 only: the subs run exits 2. write_goldens.py stopped there (fail());
    // golden write must write neither subs file and fail, and golden check must skip, its line carrying no absolute
    // path (the text folder is shown as TEXT/<slug>/<label>/subs-ocr).
    let dir = TempDir::new("subs-missing");
    let root = dir.0.as_path();
    recording(root, false);
    put(
        root,
        "disc-a/all-tracks/subs/index.json",
        SUBS_INDEX.replacen(
            "}]",
            r#"}, {"n": 1, "track_id": 4, "language": "eng", "codec": "HDMV PGS", "track_name": null, "forced": false, "default": false}]"#,
            1,
        ),
    );
    put(root, "disc-a/all-tracks/subs/sub-1.packets.json", packets());
    let text = TempDir::new("subs-missing-text");
    put(&text.0, "disc-a/all-tracks/subs-ocr/sub-0.ocr.jsonl", ocr_text());
    let ctx = ctx();
    let reason = "the subs run exited 2: stream s1 eng: no OCR text (no sub-1.ocr.jsonl in \
                  TEXT/disc-a/all-tracks/subs-ocr); read it with ocr-subs.ps1";
    let error = write(&ctx, root, Some(&text.0)).unwrap_err();
    for name in ["subs.json", "subs.txt"] {
        assert!(error.contains(&format!("failed disc-a/golden/all-tracks/{name}: {reason}\n")), "{error}");
        assert!(!root.join("disc-a/golden/all-tracks").join(name).exists(), "{name} was written");
    }
    put(root, "disc-a/golden/all-tracks/subs.json", "{}\n");
    put(root, "disc-a/golden/all-tracks/subs.txt", "x\n");
    let subs: Vec<Comparison> =
        check(&ctx, root, Some(&text.0)).into_iter().filter(|c| c.name.contains("/subs.")).collect();
    assert_eq!(
        subs,
        vec![skipped("disc-a/golden/all-tracks/subs.json", reason), skipped("disc-a/golden/all-tracks/subs.txt", reason)]
    );
    assert!(!reason.contains(&text.0.display().to_string()));
}

#[test]
fn the_proxy_golden_is_skipped_for_another_ffmpeg_an_unfetched_proxy_and_another_proxy() {
    let dir = TempDir::new("proxy");
    let root = dir.0.as_path();
    recording(root, false);
    put(root, "disc-a/golden/proxy-audio-levels-20ms-1-2/golden.json", PROXY_GOLDEN);
    let name = "disc-a/golden/proxy-audio-levels-20ms-1-2";
    let outcome = |ctx: &Ctx, text: Option<&Path>| -> Outcome {
        check(ctx, root, text).into_iter().find(|c| c.name == name).expect("the proxy comparison").outcome
    };
    let mut ctx = ctx();
    ctx.runner = answering("ffmpeg version 0.0-other");
    assert_eq!(
        outcome(&ctx, None),
        Outcome::Skipped("ffmpeg is ffmpeg version 0.0-other, the golden was written by ffmpeg version 0.0-test".to_string())
    );
    ctx.runner = answering("ffmpeg version 0.0-test");
    assert_eq!(
        outcome(&ctx, None),
        Outcome::Skipped("the proxy is not fetched: disc-a/all-tracks/audio-proxy.mka".to_string())
    );
    let text = TempDir::new("proxy-text");
    let mka = text.0.join("disc-a").join("all-tracks").join("audio-proxy.mka");
    assert_eq!(
        outcome(&ctx, Some(&text.0)),
        Outcome::Skipped(format!("the proxy is not fetched: {}", mka.display()))
    );
    put(&text.0, "disc-a/all-tracks/audio-proxy.mka", "made-up bytes, not a proxy");
    assert_eq!(
        outcome(&ctx, Some(&text.0)),
        Outcome::Skipped(format!(
            "the proxy {} has SHA-1 334c9ca93eb45d1ed5e239124a6ae671f240331d, the golden was written from \
             0000000000000000000000000000000000000000",
            mka.display()
        ))
    );
}

#[test]
fn golden_write_then_check_passes_and_a_changed_golden_fails() {
    let dir = TempDir::new("round");
    let root = dir.0.as_path();
    recording(root, true);
    let text = TempDir::new("round-text");
    put(&text.0, "disc-a/all-tracks/subs-ocr/sub-0.ocr.jsonl", ocr_text());
    let ctx = ctx();
    // The harness baseline, as lazymkv-dev retune writes it.
    let spec = fs::read_to_string(default_spec(&ctx).expect("the design spec")).unwrap();
    let req = exp13_requirements(&spec).unwrap();
    let tree = DiskTree::open(root, EVIDENCE_SHOWN).unwrap();
    let overrides = Overrides::default();
    let created = ctx.now();
    let input = HarnessInput {
        tree: &tree,
        req: &req,
        overrides: &overrides,
        previous: None,
        compare: None,
        use_cache: true,
        created: created.as_str(),
    };
    let run = harness(&input).unwrap();
    write_snapshot(&root.join(BASELINE_FILE), &serde_json::to_value(&run.snapshot).unwrap()).unwrap();

    let written = write(&ctx, root, Some(&text.0)).unwrap();
    let names = [
        "disc-a/golden/scan.json",
        "disc-a/golden/titles.txt",
        "disc-a/golden/labels.json",
        "disc-a/golden/all-tracks/reconcile.json",
        "disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.json",
        "disc-a/golden/all-tracks/audio-audio-levels-20ms-1-2.txt",
        "disc-a/golden/all-tracks/subs.json",
        "disc-a/golden/all-tracks/subs.txt",
        "disc-a/golden/all-tracks/align.json",
        "golden/retune-report.txt",
    ];
    assert_eq!(written, names.iter().map(|name| format!("wrote {name}")).collect::<Vec<_>>());
    let comparisons = check(&ctx, root, Some(&text.0));
    let mut expected: Vec<Comparison> = names.iter().map(|name| passed(name)).collect();
    expected.push(passed("audio-dataset.json"));
    // disc-a has no proxy golden: the golden table's proxy row is reported as not covered.
    expected.push(skipped(NO_PROXY, "not covered: no proxy golden"));
    expected.sort_by(|a, b| sorted_key(&a.name).cmp(&sorted_key(&b.name)));
    assert_eq!(comparisons, expected);
    assert_eq!(summary(&comparisons), ("golden check: 11 pass, 0 fail, 1 skipped".to_string(), 0));

    // The lines that state the cache and the baseline are not compared ...
    let report = root.join("golden").join("retune-report.txt");
    let edited: String = fs::read_to_string(&report)
        .unwrap()
        .lines()
        .map(|line| {
            if line.starts_with("measures: ") {
                "measures: 999 analysed now, 0 taken from the cache (the snapshot, or this run's baseline)\n".to_string()
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    fs::write(&report, edited).unwrap();
    // ... and a changed golden fails.
    let scan = root.join("disc-a").join("golden").join("scan.json");
    let changed = fs::read_to_string(&scan).unwrap().replacen("\"0:07:00\"", "\"0:07:01\"", 1);
    fs::write(&scan, changed).unwrap();
    let comparisons = check(&ctx, root, Some(&text.0));
    let not_passing: Vec<&Comparison> = comparisons.iter().filter(|c| c.outcome != Outcome::Pass).collect();
    assert_eq!(
        not_passing,
        vec![
            &skipped(NO_PROXY, "not covered: no proxy golden"),
            &Comparison {
                name: "disc-a/golden/scan.json".to_string(),
                outcome: Outcome::Fail("/titles/0/9: \"0:07:01\" != \"0:07:00\"".to_string()),
            },
        ]
    );
    assert_eq!(summary(&comparisons), ("golden check: 10 pass, 1 fail, 1 skipped".to_string(), 1));
}

/// The name of the comparison that reports a clone without a proxy golden.
const NO_PROXY: &str = "*/golden/proxy-*";

/// The order check sorts its comparisons in: component by component, each by code point (PathKey's order).
fn sorted_key(name: &str) -> Vec<String> {
    name.split('/').map(str::to_string).collect()
}

/// How many golden files a clone holds below golden/ and every <dir>/golden/: README.md, write_goldens.py and kept
/// .python files aside, a proxy-* folder counting as one.
fn golden_files(evidence: &Path) -> usize {
    fn walk(folder: &Path) -> usize {
        let Ok(entries) = fs::read_dir(folder) else {
            return 0;
        };
        entries
            .flatten()
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().is_dir() {
                    if name.starts_with("proxy-") {
                        usize::from(!name.ends_with(".python"))
                    } else {
                        walk(&entry.path())
                    }
                } else {
                    usize::from(name != "README.md" && name != "write_goldens.py" && !name.ends_with(".python"))
                }
            })
            .sum()
    }
    walk(&evidence.join("golden"))
        + fs::read_dir(evidence)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| entry.file_name() != "golden")
                    .map(|entry| walk(&entry.path().join("golden")))
                    .sum::<usize>()
            })
            .unwrap_or(0)
}

#[test]
fn the_goldens_of_the_evidence_clone_pass() {
    let ctx = Ctx::from_process();
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(evidence) => evidence,
        Err(refused) => return skip(&format!("no evidence clone: {}", refused.line)),
    };
    if !evidence.join("golden").join("README.md").is_file() {
        return skip(&format!(
            "{EVIDENCE_SHOWN} holds no goldens yet (golden/README.md; write_goldens.py writes them, plan Task 14)"
        ));
    }
    let comparisons = check(&ctx, &evidence, ocr_text_dir(&ctx, None).as_deref());
    for comparison in &comparisons {
        if let Outcome::Skipped(reason) = &comparison.outcome {
            skip(&format!("{}: {reason}", comparison.name));
        }
    }
    let walked = golden_files(&evidence);
    assert!(
        comparisons.len() >= walked,
        "{} comparison(s) for {walked} golden file(s): a golden is not compared",
        comparisons.len()
    );
    let failed: Vec<String> = comparisons
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Fail(_)))
        .map(|c| format!("{}: {}", c.name, c.outcome))
        .collect();
    assert!(failed.is_empty(), "{} golden comparison(s) fail:\n{}", failed.len(), failed.join("\n"));
    let _ = writeln!(std::io::stderr(), "{}", summary(&comparisons).0);
}
