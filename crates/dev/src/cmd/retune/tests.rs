//! The ported tests of scripts/phase0/test_audio_dataset.py's Dataset (15 of its 16: the cached-NaN test is in
//! core), NotChecked (5) and Joins (19) classes, run through `lazymkv-dev retune` on recordings written to a
//! temporary folder exactly as the Python's folder(), write_labels() and probe_file() wrote them; then (new) the
//! main corpus's report line for line against the Python, Review Focus 2, 3 and 5, the cache on and off, a v2
//! baseline, the arguments refused, the command table, the two evidence tests; last, the Counts class (2) over the
//! Python's build_count_corpus.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};

use lazymkv_core::audio::dataset::{
    destination, harness, read_snapshot, result_line, Destination, Harness, HarnessInput, Overrides,
    PreviousSnapshot, BASELINE_FILE, LEVELS_NOT_HERE, SNAPSHOT_FORMAT, SNAPSHOT_FORMAT_V2,
};
use lazymkv_core::audio::labels::{exp13_requirements, is_slug, FORMAT};
use lazymkv_core::audio::measures::{Constants, Measures};
use lazymkv_core::audio::rule::Verdict;
use lazymkv_core::recording::{decode_text, py_repr_str, Encoding, Errors, MemTree};
use lazymkv_core::testsupport::{commentary, entry, level_file_text, probe_streams_json, programme, with, PyRandom, FPS};

use super::run;
use crate::evidence::{repo_root, skip, Ctx, DiskTree, EVIDENCE_SHOWN};
use crate::pyjson::{dumps, structural_diff, Style, Tolerance};
use crate::testutil::{self, write, Scratch as TempDir};

/// Row 13 of the Python test (ROW13), with its eight numbers as {names}.
const ROW13: &str = "| 13 | Measured over whole titles on the owner's own discs, does the content rule give its commentary verdict to **no** programme track, and to at least one commentary on each of at least {verdict_discs} different discs? Before the answer may be yes, the labelled tracks must include: at least {comm} commentaries from at least {discs} discs, at least {features} of them on feature films and at least 1 with the film clearly audible under the voices; at least 1 audio-description track if the owner has a disc with one (all that the owner has, up to 3); at least {prog} programme tracks other than the reference, at least {sep} of them separately encoded lossy companions or alternate mixes; at least {centre} title whose reference has no centre channel (mono or 2.0) and at least {heavy} dialogue-heavy or speech-only title, each with an extra track. Every threshold was fitted on two tracks of one disc (`fit-disc`: the S03E16 commentary), so no track of that disc counts. | fallback |";

/// The design spec, for the two evidence tests.
const DESIGN_SPEC: &str = "docs/superpowers/specs/2026-10-04-lazymkv-design.md";

/// What the Python printed for build_main_corpus on 2026-10-13, with spec 3.4's rewritten rule: and To-label lines.
const MAIN_REPORT: &str = concat!(
    "audio dataset: corpus/owner: 3 discs, 4 titles, 4 level folders (0 measured against the wrong reference), 5 measured tracks (4 labelled), 4 references\n",
    "rule: version 1, thresholds and constants as in the code\n",
    "measures: 2 analysed now, 2 taken from the cache (the snapshot, or this run's baseline)\n",
    "\n",
    "To label (measured, no label yet; the owner answers before seeing any verdict):\n",
    "  disc-b/all-tracks stream 4: ac3 2 ch eng (audio track 4 of 4 in a player, named \"t4\")\n",
    "    lazymkv-dev labels set corpus/owner/disc-b all-tracks 4 <kind> --how <how> --saw-verdict no\n",
    "\n",
    "Kind against verdict (labelled candidates; references are not judged):\n",
    "  kind                 wanted    dup  comm   c-x r-unc  ad-p n-com  n-ad  keep   n-j  total\n",
    "  lossy-companion      keep        0     0     0     0     0     0     0     1     0      1\n",
    "  commentary           drop        0     3     0     0     0     0     0     0     0      3\n",
    "  (unlabelled)                     -     -     -     -     -     -     -     -     -      1   verdicts hidden until labelled\n",
    "  columns: dup = duplicate, comm = commentary, c-x = commentary-cross-language, r-unc = reference-unclear, ad-p = ad-pattern, n-com = note-commentary, n-ad = note-ad, keep = keep, n-j = not-judged\n",
    "\n",
    "Experiment 13 (spec section 17 row 13; tracks that count: content checked by listening before the verdict, the owner sure, each disc's packets once, never fit-disc or a disc or track in audio-fitted.json; the packaging, the menu, a flag and TheDiscDB are observations and do not count on their own, D23):\n",
    "  commentaries          1 of 8, from 1 of 4 discs; on feature films 1 of 2; film clearly audible 0 of 1 (of them partial: 0)\n",
    "  audio description     0 (1 needed if the owner has a disc with one, up to 3; checked by hand)\n",
    "  programme tracks      1 of 20 other than the reference; separately encoded 1 of 6; derived cores 0 (row 13 does not exclude them from the 20)\n",
    "  no centre channel     0 of 1 titles with a counted extra track\n",
    "  dialogue-heavy        0 of 1 titles with a counted extra track\n",
    "  score, M&E, dub, narration  isolated-score 0, music-effects 0, same-language-dub 0, alternate-narration 0 (every one the owner has; checked by hand)\n",
    "  commentary verdicts   on 1 of 3 discs\n",
    "  cross-language        commentaries 0 (commentary-cross-language 0), programme tracks 0, judged against a reference in another language; never dropped by content (addendum 9.2)\n",
    "  dubs                  0 from 0 discs; on dialogue-heavy titles 0; with the commentary pattern 0 (row 13 sets no number for dubs yet; until then such a candidate is never dropped by content)\n",
    "  swapped references    4 labelled candidates read the other way round; the programme would get the commentary verdict against 0 of them (addendum 9.3; no number in row 13)\n",
    "  false verdicts        0 (a programme track with the commentary pattern: commentary or commentary-cross-language, on any disc)\n",
    "  as it stands: not answered: too few commentaries, discs with a commentary, commentaries on feature films, commentaries with the film clearly audible, programme tracks, separately encoded programme tracks, titles whose reference has no centre channel, dialogue-heavy titles\n",
    "  not counted: disc-b 2, fit-disc 1\n",
    "\n",
    "Disagreements (labelled candidates):\n",
    "  note: disc-b/eng-only stream 3 takes its label from all-tracks stream 3 (same packets)\n",
    "\n",
    "Labels without a measurement:\n",
    "  none\n",
    "\n",
    "Folders that cannot be read:\n",
    "  none\n",
    "\n",
    "Titles whose audio is in the tuning dataset (audio-proxy.json): 1 of 5 (every successful rip and every measured title); missing: disc-b/eng-only, disc-b/extract (not measured), disc-c/t1, fit-disc/t0\n",
    "\n",
    "snapshot: corpus/owner/audio-dataset.json\n",
    "RESULT ok\n",
);

// ---------------------------------------------------------------------------------------------------------------
// Fixtures: the Python test's helpers

/// The spec text of the Python test's spec(**numbers): ROW13 with the defaults verdict_discs=3, comm=8, discs=4,
/// features=2, prog=20, sep=6, centre=1, heavy=1, each replaced when given.
fn spec_text(numbers: &[(&str, i64)]) -> String {
    let mut row = ROW13.to_string();
    for (name, default) in [("verdict_discs", 3), ("comm", 8), ("discs", 4), ("features", 2), ("prog", 20), ("sep", 6), ("centre", 1), ("heavy", 1)] {
        let value = numbers.iter().find(|(n, _)| *n == name).map_or(default, |(_, v)| *v);
        row = row.replace(&format!("{{{name}}}"), &value.to_string());
    }
    format!("# spec\n\n## 17. Experiments (phase 0)\n\n| # | Question | If no |\n|---|---|---|\n{row}\n")
}

/// The Python test's DUB_SPEC.
fn dub_spec_text() -> String {
    spec_text(&[]).replace(
        "each with an extra track.",
        "each with an extra track; at least 3 dubs in a wanted language from at least 2 discs, at least 1 of them on a dialogue-heavy title.",
    )
}

/// MIX, CENTRE, COMMENT and COPY of the Python test: a 420 s programme (seed 1), its commentary (seed 7), and a
/// quieter, 60 ms late copy.
struct Signals {
    mix: Vec<f64>,
    centre: Vec<f64>,
    comment: Vec<f64>,
    copy: Vec<f64>,
}

fn signals() -> &'static Signals {
    static SIGNALS: OnceLock<Signals> = OnceLock::new();
    SIGNALS.get_or_init(|| {
        let (mix, centre) = programme(420, 1);
        let comment = commentary(&mix, 7);
        let mut copy = vec![-100.0; 3];
        copy.extend(mix[..mix.len() - 3].iter().map(|v| v - 6.0));
        Signals { mix, centre, comment, copy }
    })
}

/// One audio stream of streams.json: (index, channels, codec, profile, language).
type Stream<'a> = (i64, i64, &'a str, Option<&'a str>, &'a str);

/// A level folder as audio-levels.ps1 writes it: the Python test's folder(label_dir, main_md5, cands, fc=True,
/// channels=6, main=1, streams=None, langs=None), as a builder.
struct Folder<'a> {
    main_md5: &'a str,
    cands: Vec<(i64, &'a [f64], &'a str)>,
    fc: bool,
    channels: i64,
    main: i64,
    streams: Option<Vec<Stream<'a>>>,
    langs: Vec<(i64, &'a str)>,
}

impl<'a> Folder<'a> {
    fn new(main_md5: &'a str, cands: &[(i64, &'a [f64], &'a str)]) -> Folder<'a> {
        Folder { main_md5, cands: cands.to_vec(), fc: true, channels: 6, main: 1, streams: None, langs: Vec::new() }
    }

    fn fc(mut self, fc: bool) -> Folder<'a> {
        self.fc = fc;
        self
    }

    fn channels(mut self, channels: i64) -> Folder<'a> {
        self.channels = channels;
        self
    }

    fn main(mut self, main: i64) -> Folder<'a> {
        self.main = main;
        self
    }

    fn streams(mut self, streams: &[Stream<'a>]) -> Folder<'a> {
        self.streams = Some(streams.to_vec());
        self
    }

    fn langs(mut self, langs: &[(i64, &'a str)]) -> Folder<'a> {
        self.langs = langs.to_vec();
        self
    }

    /// Writes audio-levels-20ms-<main>-<cands...> under `label_dir`; returns its path.
    fn write(&self, label_dir: &Path) -> PathBuf {
        let s = signals();
        let lang = |n: i64| self.langs.iter().find(|(k, _)| *k == n).map_or("eng", |(_, l)| *l);
        let mut numbers = vec![self.main.to_string()];
        numbers.extend(self.cands.iter().map(|(n, _, _)| n.to_string()));
        let d = label_dir.join(format!("audio-levels-20ms-{}", numbers.join("-")));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("main-mix.txt"), level_file_text(&s.mix)).unwrap();
        if self.fc {
            fs::write(d.join("main-fc.txt"), level_file_text(&s.centre)).unwrap();
        }
        let mut lines = vec![
            "# lazymkv audio-levels v2 (experiment 13)".to_string(),
            "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400".to_string(),
            format!(
                "main={} codec=dts profile=- channels={} layout={} language={} fc={} frames={} md5={}",
                self.main,
                self.channels,
                if self.fc { "5.1(side)" } else { "stereo" },
                lang(self.main),
                if self.fc { "yes" } else { "no" },
                s.mix.len(),
                self.main_md5
            ),
        ];
        for (n, values, md5) in &self.cands {
            fs::write(d.join(format!("cand-{n}.txt")), level_file_text(values)).unwrap();
            lines.push(format!(
                "cand={n} codec=ac3 profile=- channels=2 layout=stereo language={} frames={} md5={md5}",
                lang(*n),
                values.len()
            ));
        }
        fs::write(d.join("manifest.txt"), lines.join("\n") + "\n").unwrap();
        let streams: Vec<Stream> = match &self.streams {
            Some(streams) => streams.clone(),
            None => {
                let top = std::iter::once(self.main).chain(self.cands.iter().map(|(n, _, _)| *n)).max().unwrap();
                (1..=top)
                    .map(|k| {
                        let main = k == self.main;
                        (k, if main { self.channels } else { 2 }, if main { "dts" } else { "ac3" }, None, "eng")
                    })
                    .collect()
            }
        };
        let mut list = vec![json!({"index": 0, "codec_type": "video"})];
        for (k, channels, codec, profile, language) in &streams {
            list.push(json!({"index": k, "codec_type": "audio", "codec_name": codec, "profile": profile,
                             "channels": channels, "tags": {"language": language, "title": format!("t{k}")}}));
        }
        fs::write(d.join("streams.json"), serde_json::to_string(&json!({"streams": list})).unwrap()).unwrap();
        d
    }
}

/// The Python test's title(kind, heavy).
fn title(kind: &str, heavy: bool) -> Value {
    json!({"title_kind": kind, "dialogue_heavy": heavy, "note": ""})
}

/// The Python test's write_labels(slug_dir, tracks, titles).
fn write_labels(slug_dir: &Path, tracks: Vec<Value>, titles: Value) {
    let slug = slug_dir.file_name().unwrap().to_str().unwrap();
    let data = json!({"format": FORMAT, "slug": slug, "titles": titles, "tracks": tracks});
    write(&slug_dir.join("audio-labels.json"), serde_json::to_string(&data).unwrap());
}

/// The Python test's probe_file(path, streams): video, then the audio streams (codec, profile, channels, language).
fn probe_file(path: &Path, streams: &[(&str, Option<&str>, i64, &str)]) {
    write(path, probe_streams_json(streams));
}

/// The Python test's build_main_corpus(c): fit-disc (the disc the thresholds were fitted on); disc-b (a programme
/// copy, a commentary, an unlabelled exact duplicate, the same title ripped again eng-only with the commentary's
/// packets unchanged, a successful extract not measured yet, a failed share-drop rip); disc-c (a title measured for
/// the dataset only, no candidate).
fn build_main_corpus(c: &Path) {
    let s = signals();
    Folder::new("m0", &[(3, &s.comment[..], "c0")]).write(&c.join("fit-disc/t0"));
    write_labels(&c.join("fit-disc"), vec![entry("t0", 3, "commentary")], json!({}));
    let lv = Folder::new("mb", &[(2, &s.copy[..], "cp"), (3, &s.comment[..], "cb"), (4, &s.mix[..], "mb")])
        .write(&c.join("disc-b/all-tracks"));
    write(&lv.join("audio-proxy.json"), "{}");
    write(&c.join("disc-b/all-tracks/rip-summary.json"), r#"{"ok": true, "title": 0, "bytes": 100}"#);
    write(&c.join("disc-b/extract/extract-summary.json"), "\u{feff}{\"ok\":  true}"); // PowerShell: a byte-order mark
    write(&c.join("disc-b/share-drop/rip-summary.json"), r#"{"ok": false}"#);
    Folder::new("mb", &[(3, &s.comment[..], "cb")]).write(&c.join("disc-b/eng-only"));
    write_labels(
        &c.join("disc-b"),
        vec![entry("all-tracks", 2, "lossy-companion"), with(entry("all-tracks", 3, "commentary"), "film_audible", json!(false))],
        json!({"all-tracks": title("feature", false)}),
    );
    Folder::new("mc", &[]).fc(false).channels(2).write(&c.join("disc-c/t1"));
}

/// A fresh scratch folder (testutil's, removed when dropped) with spec.md (the default row 13); the evidence goes in
/// corpus/ below it.
fn new_tmp(tag: &str) -> TempDir {
    let tmp = TempDir::new(&format!("retune-{tag}"));
    fs::write(tmp.0.join("spec.md"), spec_text(&[])).unwrap();
    tmp
}

/// Where the evidence goes and a path below the folder as an argument.
trait TmpPaths {
    fn corpus(&self) -> PathBuf;
    fn path(&self, name: &str) -> String;
}

impl TmpPaths for TempDir {
    fn corpus(&self) -> PathBuf {
        self.0.join("corpus")
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).display().to_string()
    }
}

/// Runs `retune` with `args` alone: (exit code, stdout, stderr). The temporary folder of trial and unchecked
/// snapshots is <tmp>/tmp, so no test writes outside its own folder.
fn call(tmp: &TempDir, args: &[&str]) -> (i32, String, String) {
    let mut ctx = Ctx::for_test(&tmp.0);
    let scratch = tmp.0.join("tmp");
    fs::create_dir_all(&scratch).unwrap();
    ctx.env.insert("TMPDIR".to_string(), scratch.display().to_string());
    testutil::call(run, &ctx, args)
}

/// `retune --evidence <tmp>/corpus --spec <tmp>/spec.md <extra>`: (exit code, stdout then stderr), as the Python
/// test's run().
fn retune(tmp: &TempDir, extra: &[&str]) -> (i32, String) {
    let (corpus, spec) = (tmp.path("corpus"), tmp.path("spec.md"));
    let mut args = vec!["--evidence", corpus.as_str(), "--spec", spec.as_str()];
    args.extend_from_slice(extra);
    let (code, out, err) = call(tmp, &args);
    (code, out + &err)
}

/// The main corpus with its baseline run made (exit 0), as the Python's class corpora after setUpClass.
fn baseline_corpus(tag: &str) -> TempDir {
    let tmp = new_tmp(tag);
    build_main_corpus(&tmp.corpus());
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    tmp
}

/// baseline_corpus and the baseline's text. Py: NotChecked.setUp.
fn checked_corpus(tag: &str) -> (TempDir, String) {
    let tmp = baseline_corpus(tag);
    let baseline = fs::read_to_string(tmp.corpus().join(BASELINE_FILE)).unwrap();
    (tmp, baseline)
}

/// The text after `heading` up to the next blank line. Py: section.
fn section(out: &str, heading: &str) -> String {
    let after = out.split_once(heading).map(|(_, rest)| rest).unwrap_or_else(|| panic!("no {heading:?} in:\n{out}"));
    after.split_once("\n\n").map_or(after, |(head, _)| head).to_string()
}

/// {first word: the line's words} of the Kind against verdict table. Py: the tests' `rows`.
fn table_rows(out: &str) -> BTreeMap<String, Vec<String>> {
    section(out, "Kind against verdict")
        .lines()
        .skip(2)
        .map(|line| {
            let words: Vec<String> = line.split_whitespace().map(String::from).collect();
            (words[0].clone(), words)
        })
        .collect()
}

fn words(items: &[String]) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

/// assertRegex(out, r"measures: [1-9]\d* analysed now").
fn analysed_now(out: &str) -> bool {
    out.lines().any(|line| {
        line.strip_prefix("measures: ")
            .and_then(|rest| rest.split_once(" analysed now"))
            .is_some_and(|(n, _)| !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries.map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// Every */*/audio-levels-*/*.txt below `corpus` (the files git does not keep).
fn level_files(corpus: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for slug in subdirs(corpus) {
        for label in subdirs(&slug) {
            for folder in subdirs(&label) {
                if !folder.file_name().unwrap().to_string_lossy().starts_with("audio-levels-") {
                    continue;
                }
                for entry in fs::read_dir(&folder).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "txt") {
                        out.push(path);
                    }
                }
            }
        }
    }
    out
}

fn read_json_file(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// The report lines without the measures: line (the only one that depends on the cache).
fn strip_measures(lines: &[String]) -> Vec<&String> {
    lines.iter().filter(|line| !line.starts_with("measures: ")).collect()
}

fn value_of(h: &Harness) -> Value {
    serde_json::to_value(&h.snapshot).unwrap()
}

/// The first run over the main corpus, shared by the five Dataset tests that only read it (the Python's setUpClass).
struct MainRun {
    code: i32,
    out: String,
    snapshot: Value,
}

fn main_run() -> &'static MainRun {
    static RUN: OnceLock<MainRun> = OnceLock::new();
    RUN.get_or_init(|| {
        let tmp = new_tmp("main");
        build_main_corpus(&tmp.corpus());
        let (code, out) = retune(&tmp, &[]);
        let snapshot = read_json_file(&tmp.corpus().join(BASELINE_FILE));
        MainRun { code, out, snapshot }
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Dataset

#[test]
fn the_baseline_run_passes_and_writes_its_snapshot() {
    let run = main_run();
    assert_eq!(run.code, 0, "{}", run.out);
    assert!(run.out.trim_end().ends_with("RESULT ok"), "{}", run.out);
    let snap = &run.snapshot;
    assert_eq!(snap["format"], SNAPSHOT_FORMAT);
    let tracks = snap["tracks"].as_array().unwrap();
    let find = |slug: &str, label: &str, stream: i64| {
        tracks.iter().find(|t| t["slug"] == slug && t["label"] == label && t["stream"] == stream).unwrap()
    };
    let comm = find("disc-b", "all-tracks", 3);
    assert_eq!((&comm["verdict"], &comm["kind"], &comm["counts"]), (&json!("commentary"), &json!("commentary"), &json!(true)));
    assert_eq!(comm["measures"], snap["measures_cache"][comm["key"].as_str().unwrap()]);
    let dup = find("disc-b", "all-tracks", 4);
    assert!(dup["verdict"].is_null()); // unlabelled: its verdict is kept apart
    assert_eq!(snap["unlabelled_verdicts"], json!({"disc-b/all-tracks/audio-levels-20ms-1-2-3-4/candidate/4": "duplicate"}));
}

#[test]
fn the_owners_to_do_list_shows_no_verdict() {
    let run = main_run();
    let todo = section(&run.out, "To label (measured, no label yet");
    assert!(todo.contains("disc-b/all-tracks stream 4: ac3 2 ch eng (audio track 4 of 4 in a player, named \"t4\")"), "{todo}");
    assert!(todo.contains("lazymkv-dev labels set corpus/owner/disc-b all-tracks 4 <kind>"), "{todo}");
    let bare = todo.replace("<kind>", "");
    for v in Verdict::ALL {
        assert!(!bare.contains(v.as_str()), "{} in {todo}", v.as_str());
    }
    assert!(!todo.contains("eng-only"), "{todo}"); // its stream 3 has the labelled track's packets: the label carries over
}

#[test]
fn kind_against_verdict() {
    let rows = table_rows(&main_run().out);
    assert_eq!(words(&rows["commentary"][2..]), ["0", "3", "0", "0", "0", "0", "0", "0", "0", "3"]); // fit-disc, disc-b twice
    let lossy = &rows["lossy-companion"];
    assert_eq!(words(&lossy[lossy.len() - 2..]), ["0", "1"]);
    assert!(rows["(unlabelled)"].join(" ").contains("verdicts hidden until labelled"));
    assert_eq!(words(&rows["(unlabelled)"][1..9]), ["-"; 8]);
}

#[test]
fn experiment_13_counts_each_track_once_and_never_the_fitting_disc() {
    let e = section(&main_run().out, "Experiment 13 (");
    assert!(e.contains("commentaries          1 of 8, from 1 of 4 discs; on feature films 1 of 2; film clearly audible 0 of 1"), "{e}");
    assert!(e.contains("programme tracks      1 of 20 other than the reference; separately encoded 1 of 6"), "{e}");
    assert!(e.contains("commentary verdicts   on 1 of 3 discs"), "{e}");
    assert!(e.contains("false verdicts        0"), "{e}");
    assert!(e.contains("as it stands: not answered"), "{e}");
    assert!(e.contains("not counted: disc-b 2, fit-disc 1"), "{e}"); // the unlabelled duplicate and the re-ripped commentary
}

#[test]
fn titles_missing_from_the_dataset() {
    let out = &main_run().out;
    assert!(out.contains("Labels without a measurement:\n  none"), "{out}");
    assert!(out.contains("Folders that cannot be read:\n  none"), "{out}");
    // not share-drop: it failed
    assert!(out.contains("1 of 5 (every successful rip and every measured title); missing: disc-b/eng-only, disc-b/extract (not measured), disc-c/t1, fit-disc/t0"), "{out}");
}

#[test]
fn a_second_run_takes_every_measure_from_the_snapshot() {
    let tmp = baseline_corpus("second");
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("measures: 0 analysed now, 4 taken from the cache"), "{out}");
    assert!(out.contains("Changes since the previous snapshot"), "{out}");
    let block = section(&out, "Changes since the previous snapshot");
    let line = block.lines().nth(1).unwrap();
    assert!("  none".contains(line), "{block}");
}

#[test]
fn a_run_that_changes_nothing_leaves_the_baseline_file_as_it_is() {
    let tmp = baseline_corpus("unchanged");
    let base = tmp.corpus().join(BASELINE_FILE);
    let mut data = read_json_file(&base);
    // as if the run had been made earlier (the Python also set "corpus" to the copy's path; the Rust's is always
    // "corpus/owner", so it stays)
    data["created"] = json!("2000-01-01T00:00:00");
    fs::write(&base, dumps(&data, Style::INDENT1_ASCII) + "\n").unwrap();
    let before = fs::read_to_string(&base).unwrap();
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("snapshot: corpus/owner/audio-dataset.json (unchanged but for its time: not rewritten)"), "{out}");
    assert_eq!(fs::read_to_string(&base).unwrap(), before); // not even its "created"
    write_labels(
        &tmp.corpus().join("disc-b"),
        vec![
            entry("all-tracks", 2, "lossy-companion"),
            with(entry("all-tracks", 3, "commentary"), "film_audible", json!(false)),
            with(entry("all-tracks", 4, "duplicate"), "how", json!("md5")),
        ],
        json!({"all-tracks": title("feature", false)}),
    );
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("snapshot: corpus/owner/audio-dataset.json\n"), "{out}");
    assert_ne!(read_json_file(&base)["created"], "2000-01-01T00:00:00"); // a new label
}

#[test]
fn a_rule_trial_reuses_the_baselines_measures() {
    let tmp = baseline_corpus("rule-trial");
    fs::remove_file(tmp.corpus().join(BASELINE_FILE)).unwrap();
    let t = tmp.path("t.json");
    let (code, out) = retune(&tmp, &["--set", "DC_MAX=0.2", "--snapshot", t.as_str()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("measures: 2 analysed now, 4 taken from the cache"), "{out}"); // the trial analyses nothing again
}

#[test]
fn a_threshold_trial_lists_changes_and_leaves_the_baseline_alone() {
    let tmp = baseline_corpus("threshold");
    let base = tmp.corpus().join(BASELINE_FILE);
    let before = fs::read_to_string(&base).unwrap();
    let trial = tmp.path("trial.json");
    let (code, out) = retune(&tmp, &["--set", "DC_MAX=0.0", "--snapshot", trial.as_str()]);
    assert_eq!(code, 0, "{out}");
    let changes = section(&out, "Changes against the baseline rule (DC_MAX=0.0)");
    assert!(changes.contains("disc-b/all-tracks stream 3 in audio-levels-20ms-1-2-3-4 (commentary): commentary -> note-commentary"), "{changes}");
    assert!(out.contains("kept although unwanted, commentary"), "{out}");
    assert_eq!(fs::read_to_string(&base).unwrap(), before);
    assert_eq!(read_json_file(Path::new(&trial))["overrides"]["rule"], json!({"dc_max": 0.0}));
}

#[test]
fn a_trial_never_shows_an_unlabelled_tracks_verdict() {
    let tmp = baseline_corpus("hidden");
    write_labels(&tmp.corpus().join("disc-b"), vec![entry("all-tracks", 2, "lossy-companion")], json!({}));
    let t = tmp.path("t.json");
    let (code, out) = retune(&tmp, &["--set", "DC_MAX=0.0", "--snapshot", t.as_str()]);
    assert_eq!(code, 0, "{out}");
    let changes = section(&out, "Changes against the baseline rule");
    assert!(changes.contains("2 unlabelled track(s) changed verdict (not shown until labelled)"), "{changes}"); // fit-disc is labelled
    let rest = changes.replace("fit-disc/t0 stream 3 in audio-levels-20ms-1-3 (commentary): commentary -> note-commentary", "");
    assert!(!rest.contains("note-commentary"), "{changes}");
}

#[test]
fn a_constant_trial_measures_again_and_bad_values_are_refused() {
    let tmp = baseline_corpus("constant");
    let t2 = tmp.path("t2.json");
    let (code, out) = retune(&tmp, &["--set", "ADDED_DB=40", "--snapshot", t2.as_str()]);
    assert_eq!(code, 0, "{out}");
    assert!(analysed_now(&out), "{out}");
    assert!(out.contains("commentary -> keep"), "{out}");
    // sim.ADDED_DB restored: the Rust's constants are values handed in, never module state
    assert_eq!(Constants::default().ADDED_DB, 10.0);
    for bad in ["NOPE=1", "DC_MAX=nan", "SILENT_DB=inf", "DC_MAX=-inf", "TENTHS_MIN=1.5", "DC_MAX=x", "DC_MAX"] {
        let (code, out) = retune(&tmp, &["--set", bad]);
        assert_eq!(code, 2, "{bad}: {out}");
    }
    assert!(retune(&tmp, &["--set", "DC_MAX=nan"]).1.contains("not a finite number"));
}

/// `--set <name>=-1` for a lag constant: the Python's max() over an empty range raised "max() arg is an empty
/// sequence", which audio_dataset.py caught, so every folder with a candidate was listed as unreadable and the run
/// exited 3. Pinned to d3a754d's audio_dataset.main on the Python test's main corpus, for each of the three constants.
fn a_negative_lag_constant_lists_the_folders_as_unreadable(name: &str) {
    let (tmp, baseline) = checked_corpus(&format!("lag-{}", name.to_lowercase()));
    let (set, trial) = (format!("{name}=-1"), tmp.path("trial.json"));
    let (code, out) = retune(&tmp, &["--set", set.as_str(), "--snapshot", trial.as_str()]);
    assert_eq!(code, 3, "{out}");
    assert_eq!(
        section(&out, "Folders that cannot be read:"),
        "\n  disc-b/all-tracks/audio-levels-20ms-1-2-3-4: max() arg is an empty sequence\
         \n  disc-b/eng-only/audio-levels-20ms-1-3: max() arg is an empty sequence\
         \n  fit-disc/t0/audio-levels-20ms-1-3: max() arg is an empty sequence",
        "{out}"
    );
    assert!(out.contains("RESULT FAILED: not everything was checked: 3 folder(s) cannot be read, 3 labelled track(s) not measured (see 'Folders that cannot be read', 'Labels without a measurement' and the changes above)"), "{out}");
    assert_eq!(fs::read_to_string(tmp.corpus().join(BASELINE_FILE)).unwrap(), baseline);
}

#[test]
fn a_negative_coarse_lag_lists_the_folders_as_unreadable() {
    a_negative_lag_constant_lists_the_folders_as_unreadable("COARSE_LAG");
}

#[test]
fn a_negative_block_lag_lists_the_folders_as_unreadable() {
    a_negative_lag_constant_lists_the_folders_as_unreadable("BLOCK_LAG");
}

#[test]
fn a_negative_win_lag_lists_the_folders_as_unreadable() {
    a_negative_lag_constant_lists_the_folders_as_unreadable("WIN_LAG");
}

#[test]
fn a_programme_track_with_the_commentary_verdict_fails_loudly() {
    let tmp = baseline_corpus("fail-commentary");
    write_labels(&tmp.corpus().join("disc-b"), vec![entry("all-tracks", 3, "alternate-mix")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-b/all-tracks stream 3: labelled alternate-mix (programme audio), verdict commentary"), "{out}");
    assert!(out.contains("FAIL disc-b/eng-only stream 3: labelled alternate-mix (programme audio), verdict commentary (label of all-tracks stream 3)"), "{out}");
    assert!(out.contains("RESULT FAILED: 2 programme track(s) got a drop verdict"), "{out}"); // and its re-rip, by the same packets
    assert!(out.contains("false verdicts        2"), "{out}");
    assert!(out.contains("as it stands: no: 2 programme track(s)"), "{out}");
}

#[test]
fn a_programme_track_with_the_duplicate_verdict_fails_loudly() {
    let tmp = baseline_corpus("fail-duplicate");
    write_labels(&tmp.corpus().join("disc-b"), vec![entry("all-tracks", 4, "lossy-companion")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-b/all-tracks stream 4: labelled lossy-companion (programme audio), verdict duplicate"), "{out}");
    assert!(out.contains("false verdicts        0"), "{out}");
}

#[test]
fn undecided_and_unknown_kinds_with_a_drop_verdict_are_notes() {
    let tmp = baseline_corpus("notes");
    write_labels(
        &tmp.corpus().join("disc-b"),
        vec![
            entry("all-tracks", 3, "same-language-dub"),
            entry("all-tracks", 4, "unknown"),
            entry("all-tracks", 2, "isolated-score"),
            entry("eng-only", 3, "isolated-score"),
        ],
        json!({}),
    );
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("note: disc-b/all-tracks stream 3: undecided kind, drop verdict (same-language-dub, verdict commentary)"), "{out}");
    assert!(out.contains("note: disc-b/all-tracks stream 4: unknown kind, drop verdict (unknown, verdict duplicate)"), "{out}");
    assert!(out.contains("kept although unwanted, isolated-score: disc-b/all-tracks stream 2 (keep)"), "{out}");
    assert!(out.contains("note: disc-b/eng-only stream 3: dropped as a commentary (isolated-score, verdict commentary)"), "{out}");
}

#[test]
fn a_bad_labels_or_fitted_file_stops_the_run() {
    let tmp = baseline_corpus("bad-labels");
    write_labels(&tmp.corpus().join("fit-disc"), vec![entry("t0", 3, "comentary")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("not in the vocabulary"), "{out}");
    assert!(out.ends_with("RESULT FAILED: a labels file does not validate (lazymkv-dev labels check)\n"), "{out}");
    write_labels(&tmp.corpus().join("fit-disc"), vec![entry("t0", 3, "commentary")], json!({}));
    write(&tmp.corpus().join("audio-fitted.json"), r#"{"format": "lazymkv audio fitted v1", "fitted": [{"slug": "Disc"}]}"#);
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("audio-fitted.json"), "{out}");
}

// ---------------------------------------------------------------------------------------------------------------
// NotChecked: runs that cannot check everything end with exit 3 and never replace the baseline snapshot

#[test]
fn level_files_missing_as_after_a_clone() {
    let (tmp, baseline) = checked_corpus("clone");
    for f in level_files(&tmp.corpus()) {
        fs::remove_file(f).unwrap();
    }
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("RESULT FAILED: not everything was checked: 4 folder(s) cannot be read, 3 labelled track(s) not measured, 9 track(s) of the previous snapshot are gone"), "{out}");
    assert!(out.contains("  gone: 9 track(s) not measured now: disc-b/all-tracks/audio-levels-20ms-1-2-3-4/candidate/2"), "{out}");
    assert!(out.contains("  disc-b/eng-only/audio-levels-20ms-1-3: its level files are not here (git keeps only streams.json)"), "{out}");
    assert!(out.contains("missing: disc-b/eng-only, disc-b/extract (not measured), disc-c/t1, fit-disc/t0"), "{out}");
    assert!(out.contains("was NOT replaced"), "{out}");
    assert_eq!(fs::read_to_string(tmp.corpus().join(BASELINE_FILE)).unwrap(), baseline);
    let (code, out) = retune(&tmp, &["--allow-unmeasured"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("RESULT ok (not checked, allowed by --allow-unmeasured: 4 folder(s) cannot be read"), "{out}");
    assert_eq!(fs::read_to_string(tmp.corpus().join(BASELINE_FILE)).unwrap(), baseline);
}

#[test]
fn no_corpus_folder_as_without_the_evidence_clone() {
    let tmp = new_tmp("no-clone");
    let missing = tmp.0.join("owner");
    let shown = missing.display().to_string();
    let spec = tmp.path("spec.md");
    for extra in [&[][..], &["--allow-unmeasured"][..]] {
        let mut args = vec!["--evidence", shown.as_str(), "--spec", spec.as_str()];
        args.extend_from_slice(extra);
        let (code, out, err) = call(&tmp, &args);
        assert_eq!(code, 3, "{err}");
        assert_eq!(out, "");
        assert_eq!(err, format!("{shown}: no such folder; the evidence repository lazymkv-evidence is not cloned there (git clone https://github.com/krcm0209/lazymkv-evidence {shown})\n"));
    }
    assert!(!missing.exists());
    fs::write(&missing, "").unwrap(); // a file in its place is a usage error, as before
    let (code, out, err) = call(&tmp, &["--evidence", shown.as_str(), "--spec", spec.as_str()]);
    assert_eq!((code, out + &err), (2, format!("{shown}: not a folder\n")));
}

#[test]
fn no_level_folder_at_all() {
    let tmp = new_tmp("empty");
    let empty = tmp.path("empty");
    fs::create_dir_all(&empty).unwrap();
    let spec = tmp.path("spec.md");
    let (code, out, err) = call(&tmp, &["--evidence", empty.as_str(), "--spec", spec.as_str()]);
    assert_eq!(code, 3, "{out}{err}");
    assert!(out.contains("no level folder under"), "{out}");
}

#[test]
fn a_label_without_a_measurement() {
    let (tmp, _) = checked_corpus("orphan");
    let data = read_json_file(&tmp.corpus().join("disc-b/audio-labels.json"));
    let mut tracks = data["tracks"].as_array().unwrap().clone();
    tracks.push(entry("all-tracks", 7, "foreign-dub"));
    write_labels(&tmp.corpus().join("disc-b"), tracks, data["titles"].clone());
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("Labels without a measurement:\n  disc-b/all-tracks stream 7: foreign-dub"), "{out}");
    assert!(out.contains("1 labelled track(s) not measured"), "{out}");
    let (code, out) = retune(&tmp, &["--allow-unmeasured"]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_folder_that_is_gone() {
    let (tmp, baseline) = checked_corpus("gone");
    fs::remove_dir_all(tmp.corpus().join("disc-b/eng-only")).unwrap();
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("gone: 2 track(s) not measured now: disc-b/eng-only/audio-levels-20ms-1-3/candidate/3, disc-b/eng-only/audio-levels-20ms-1-3/reference/1"), "{out}");
    assert_eq!(fs::read_to_string(tmp.corpus().join(BASELINE_FILE)).unwrap(), baseline);
    let base = tmp.corpus().join(BASELINE_FILE).display().to_string();
    let (code, out) = retune(&tmp, &["--snapshot", base.as_str(), "--allow-unmeasured"]);
    assert_eq!(code, 0, "{out}"); // the removal accepted on purpose
    assert_eq!(retune(&tmp, &[]).0, 0);
}

// ---------------------------------------------------------------------------------------------------------------
// Joins: labels carried over by identical packets, several folders of one title, and the reference check

#[test]
fn a_rerip_of_a_title_with_duplicates_takes_each_label_from_the_same_copy() {
    let tmp = new_tmp("rerip");
    let (s, c) = (signals(), tmp.corpus());
    for label in ["rehearsal", "rehearsal-512"] {
        // streams 1 and 3, 2 and 4 have the same packets
        Folder::new("A", &[(2, &s.copy[..], "B"), (3, &s.mix[..], "A"), (4, &s.copy[..], "B")]).write(&c.join("disc-r").join(label));
    }
    write_labels(
        &c.join("disc-r"),
        vec![
            entry("rehearsal", 2, "derived-core"),
            with(entry("rehearsal", 3, "duplicate"), "how", json!("md5")),
            with(entry("rehearsal", 4, "duplicate"), "how", json!("md5")),
        ],
        json!({}),
    );
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("FAIL"), "{out}");
    for n in [2, 3, 4] {
        assert!(out.contains(&format!("note: disc-r/rehearsal-512 stream {n} takes its label from rehearsal stream {n} (same packets)")), "{out}");
    }
    assert!(!out.contains("reference labelled"), "{out}");
    assert!(out.contains("programme tracks      1 of 20"), "{out}"); // the core counts once, not again from the re-rip
}

#[test]
fn disagreeing_sources_leave_the_track_to_be_labelled() {
    let tmp = new_tmp("disagree");
    let (s, c) = (signals(), tmp.corpus());
    for label in ["all-tracks", "again", "third"] {
        Folder::new("A", &[(2, &s.copy[..], "B")]).write(&c.join("disc-r").join(label));
    }
    write_labels(&c.join("disc-r"), vec![entry("all-tracks", 2, "lossy-companion"), entry("again", 2, "alternate-mix")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("disc-r/third stream 2: ac3 2 ch eng (audio track 2 of 2 in a player, named \"t2\"); same packets as again stream 2 (alternate-mix), all-tracks stream 2 (lossy-companion): they disagree"), "{out}");
}

#[test]
fn a_reference_never_takes_a_label() {
    let tmp = new_tmp("reference-label");
    let (s, c) = (signals(), tmp.corpus());
    Folder::new("A", &[(2, &s.copy[..], "B")]).write(&c.join("disc-r/all-tracks"));
    Folder::new("B", &[]).write(&c.join("disc-r/eng-only")); // only stream B was ripped: it is the reference here
    write_labels(&c.join("disc-r"), vec![entry("all-tracks", 2, "lossy-companion")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("eng-only stream 1: reference labelled"), "{out}");
}

#[test]
fn every_folder_of_a_title_is_judged() {
    let tmp = new_tmp("every-folder");
    let (s, c) = (signals(), tmp.corpus());
    Folder::new("w1", &[(2, &s.copy[..], "w2"), (3, &s.comment[..], "w3")]).write(&c.join("disc-w/t0"));
    Folder::new("w1", &[(3, &s.comment[..], "w3")]).write(&c.join("disc-w/t0"));
    write_labels(&c.join("disc-w"), vec![entry("t0", 3, "alternate-mix")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-w/t0 stream 3 (in audio-levels-20ms-1-2-3): labelled alternate-mix"), "{out}");
    assert!(out.contains("FAIL disc-w/t0 stream 3 (in audio-levels-20ms-1-3): labelled alternate-mix"), "{out}");
    assert!(out.contains("note: disc-w/t0 stream 3 (candidate) is measured in audio-levels-20ms-1-2-3, audio-levels-20ms-1-3; the table uses audio-levels-20ms-1-2-3"), "{out}");
    assert_eq!(table_rows(&out)["alternate-mix"].last().unwrap(), "1"); // one track, judged twice
}

#[test]
fn a_folder_measured_against_the_wrong_reference_fails_and_is_left_out() {
    let tmp = new_tmp("wrong-reference");
    let (s, c) = (signals(), tmp.corpus());
    let streams: [Stream; 3] = [(1, 6, "dts", Some("DTS-HD MA"), "eng"), (2, 6, "dts", Some("DTS"), "eng"), (3, 2, "ac3", None, "eng")];
    Folder::new("w1", &[(2, &s.copy[..], "w2"), (3, &s.comment[..], "w3")]).streams(&streams).write(&c.join("disc-w/t0"));
    Folder::new("w3", &[(1, &s.mix[..], "w1"), (2, &s.copy[..], "w2")]).main(3).streams(&streams).write(&c.join("disc-w/t0"));
    write_labels(&c.join("disc-w"), vec![with(entry("t0", 2, "derived-core"), "how", json!("flag")), entry("t0", 3, "commentary")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-w/t0/audio-levels-20ms-3-1-2: wrong reference: measured against stream 3 (ac3 2 ch t3), but the top-ranked eng stream is 1 (dts DTS-HD MA 6 ch t1)"), "{out}");
    assert!(out.contains("RESULT FAILED: 1 folder(s) measured against the wrong reference"), "{out}");
    let rows = table_rows(&out);
    assert_eq!((rows["commentary"].last().unwrap().as_str(), rows["derived-core"].last().unwrap().as_str()), ("1", "1"));
    assert!(out.contains("(1 measured against the wrong reference)"), "{out}");
}

#[test]
fn a_reference_labelled_as_unwanted_audio_fails() {
    let tmp = new_tmp("unwanted-reference");
    let (s, c) = (signals(), tmp.corpus());
    Folder::new("w1", &[(2, &s.copy[..], "w2")]).write(&c.join("disc-w/t0"));
    write_labels(&c.join("disc-w"), vec![entry("t0", 1, "commentary"), entry("t0", 2, "lossy-companion")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-w/t0 stream 1: the reference is labelled commentary (unwanted audio)"), "{out}");
}

#[test]
fn a_commentary_in_another_language_is_judged_against_the_original_language() {
    let tmp = new_tmp("cross-language");
    let (s, c) = (signals(), tmp.corpus());
    let streams: [Stream; 3] = [(1, 6, "dts", Some("DTS-HD MA"), "spa"), (2, 6, "dts", Some("DTS"), "spa"), (3, 2, "ac3", None, "eng")];
    Folder::new("s1", &[(2, &s.copy[..], "s2"), (3, &s.comment[..], "s3")])
        .streams(&streams)
        .langs(&[(1, "spa"), (2, "spa")])
        .write(&c.join("disc-s/t0"));
    write(&c.join("disc-s/tmdb.json"), r#"{"original_language": "es"}"#);
    write_labels(
        &c.join("disc-s"),
        vec![with(entry("t0", 2, "derived-core"), "how", json!("flag")), entry("t0", 3, "commentary")],
        json!({"t0": title("feature", false)}),
    );
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("(0 measured against the wrong reference)"), "{out}");
    assert!(out.contains("cross-language        commentaries 1 (commentary-cross-language 1), programme tracks 0"), "{out}");
    assert_eq!(words(&table_rows(&out)["commentary"][2..5]), ["0", "0", "1"]); // the pattern, in another language: keep-only
    assert!(out.contains("commentary verdicts   on 0 of 3 discs"), "{out}"); // only a verdict that may drop shows the recall
    assert!(out.contains("kept although unwanted, commentary: disc-s/t0 stream 3 (commentary-cross-language)"), "{out}");
    assert!(out.contains("swapped references    2 labelled candidates read the other way round; the programme would get the commentary verdict against 0 of them"), "{out}");
}

#[test]
fn a_folder_against_the_english_track_of_a_spanish_film_is_the_wrong_reference() {
    let tmp = new_tmp("english-of-spanish");
    let (s, c) = (signals(), tmp.corpus());
    let streams: [Stream; 3] = [(1, 6, "dts", Some("DTS-HD MA"), "spa"), (2, 2, "ac3", None, "eng"), (3, 2, "ac3", None, "eng")];
    Folder::new("s2", &[(3, &s.comment[..], "s3")]).main(2).streams(&streams).write(&c.join("disc-s/t0"));
    write(&c.join("disc-s/tmdb.json"), r#"{"original_language": "es"}"#);
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("wrong reference: measured against stream 2 (ac3 2 ch t2), but the top-ranked spa stream is 1 (dts DTS-HD MA 6 ch t1), the title's programme reference (placed first in the original language"), "{out}");
}

#[test]
fn a_dub_is_programme_audio() {
    let tmp = new_tmp("dub");
    let (s, c) = (signals(), tmp.corpus());
    Folder::new("u1", &[(2, &s.comment[..], "u2")])
        .langs(&[(1, "spa")])
        .streams(&[(1, 6, "dts", Some("DTS-HD MA"), "spa"), (2, 2, "ac3", None, "eng")])
        .write(&c.join("disc-u/t0"));
    write(&c.join("disc-u/tmdb.json"), r#"{"original_language": "es"}"#);
    write_labels(&c.join("disc-u"), vec![entry("t0", 2, "dub")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-u/t0 stream 2: labelled dub (programme audio), verdict commentary-cross-language"), "{out}");
    assert!(out.contains("cross-language        commentaries 0 (commentary-cross-language 0), programme tracks 1"), "{out}");
    assert!(out.contains("dubs                  1 from 1 discs; on dialogue-heavy titles 0; with the commentary pattern 1 (row 13 sets no number for dubs yet"), "{out}");
    assert!(out.contains("false verdicts        1"), "{out}"); // kept, but the pattern fired on programme audio
    fs::write(tmp.0.join("dub-spec.md"), dub_spec_text()).unwrap();
    let (corpus, dub_spec) = (tmp.path("corpus"), tmp.path("dub-spec.md"));
    let (_, out, err) = call(&tmp, &["--evidence", corpus.as_str(), "--spec", dub_spec.as_str()]);
    assert!(out.contains("dubs                  1 of 3 from 1 of 2 discs; on dialogue-heavy titles 0 of 1; with the commentary pattern 1 (what row 13 asks before content may judge a candidate in another language"), "{out}{err}");
}

#[test]
fn a_dub_is_not_a_separately_encoded_programme_track() {
    let tmp = new_tmp("dub-separate");
    let (s, c) = (signals(), tmp.corpus());
    let quieter: Vec<f64> = s.mix.iter().map(|v| v - 6.0).collect();
    Folder::new("u1", &[(2, &quieter[..], "u2")])
        .langs(&[(1, "spa")])
        .streams(&[(1, 6, "dts", Some("DTS-HD MA"), "spa"), (2, 2, "ac3", None, "eng")])
        .write(&c.join("disc-u/t0"));
    write(&c.join("disc-u/tmdb.json"), r#"{"original_language": "es"}"#);
    write_labels(&c.join("disc-u"), vec![entry("t0", 2, "dub")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("programme tracks      1 of 20 other than the reference; separately encoded 0 of 6"), "{out}");
}

#[test]
fn a_rip_without_an_audio_stream_is_noted_not_missing() {
    let tmp = new_tmp("no-audio");
    let (s, c) = (signals(), tmp.corpus());
    Folder::new("v1", &[(2, &s.copy[..], "v2")]).write(&c.join("disc-v/t0"));
    write(&c.join("disc-v/t0/audio-levels-20ms-1-2/audio-proxy.json"), "{}");
    let none: &[(&str, Option<&str>, i64, &str)] = &[];
    for (label, streams) in [("rehearsal", none), ("again", &[("ac3", None, 2, "eng")][..])] {
        probe_file(&c.join("disc-v").join(label).join("ffprobe-streams.json"), streams);
        write(&c.join("disc-v").join(label).join("rip-summary.json"), r#"{"ok": true, "title": 1}"#);
    }
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Titles whose audio is in the tuning dataset (audio-proxy.json): 1 of 2 (every successful rip and every measured title); missing: disc-v/again (not measured)"), "{out}");
    assert!(out.contains("  note: no audio stream (its ffprobe-streams.json lists none), so nothing to add and not missing: disc-v/rehearsal"), "{out}");
}

#[test]
fn a_swapped_reference_that_would_drop_the_programme_fails() {
    let tmp = new_tmp("swapped");
    let (s, c) = (signals(), tmp.corpus());
    // a stem: music cues of 12 s at -30 dB in every 30 s, room tone between them
    let mut rnd = PyRandom::new(5);
    let stem: Vec<f64> = (0..s.mix.len())
        .map(|t| (if (t / FPS) % 30 < 12 { -30.0 } else { -62.0 }) + rnd.uniform(-1.0, 1.0))
        .collect();
    Folder::new("m1", &[(2, &stem[..], "m2")]).write(&c.join("disc-m/t0"));
    write_labels(&c.join("disc-m"), vec![entry("t0", 2, "music-effects")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-m/t0 stream 2: swapped reference: labelled music-effects, verdict reference-unclear; with this track as the reference, the programme (stream 1) would get the commentary verdict"), "{out}");
    assert!(out.contains("RESULT FAILED: 1 swapped reference(s) would drop the programme"), "{out}");
    write_labels(&c.join("disc-m"), vec![], json!({})); // unlabelled: neither the verdict nor the swapped one is shown
    let t = c.join("t.json").display().to_string();
    let (code, out) = retune(&tmp, &["--snapshot", t.as_str()]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("swapped reference:"), "{out}");
}

#[test]
fn the_companion_of_a_dub_is_judged_against_the_dub_too() {
    let tmp = new_tmp("companion");
    let (s, c) = (signals(), tmp.corpus());
    let mut companion = vec![-100.0; 3]; // the dub stand-in has the pattern (addendum 9.2)
    companion.extend(s.comment[..s.comment.len() - 3].iter().map(|v| v - 6.0));
    let streams: [Stream; 3] = [(1, 6, "dts", Some("DTS-HD MA"), "spa"), (2, 2, "ac3", None, "eng"), (3, 2, "ac3", None, "eng")];
    Folder::new("x1", &[(2, &s.comment[..], "x2"), (3, &companion[..], "x3")])
        .streams(&streams)
        .langs(&[(1, "spa")])
        .write(&c.join("disc-x/t0"));
    write(&c.join("disc-x/tmdb.json"), r#"{"original_language": "es"}"#);
    write_labels(&c.join("disc-x"), vec![entry("t0", 2, "dub"), entry("t0", 3, "lossy-companion")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-x/t0 stream 2: labelled dub (programme audio), verdict commentary-cross-language"), "{out}");
    assert!(!out.contains("FAIL disc-x/t0 stream 3"), "{out}");
    let snap = read_json_file(&c.join(BASELINE_FILE));
    let rows: BTreeMap<i64, Value> =
        snap["tracks"].as_array().unwrap().iter().map(|t| (t["stream"].as_i64().unwrap(), t.clone())).collect();
    assert_eq!((&rows[&3]["verdict"], &rows[&3]["peer"]["stream"], &rows[&3]["peer"]["verdict"]),
               (&json!("note-commentary"), &json!(2), &json!("keep")));
    assert!(rows[&2]["peer"].is_null());
}

#[test]
fn the_snapshot_hides_every_verdict_of_an_unlabelled_track() {
    let tmp = new_tmp("hide");
    let (s, c) = (signals(), tmp.corpus());
    let mut companion = vec![-100.0; 3];
    companion.extend(s.comment[..s.comment.len() - 3].iter().map(|v| v - 6.0));
    let streams: [Stream; 3] = [(1, 6, "dts", Some("DTS-HD MA"), "spa"), (2, 2, "ac3", None, "eng"), (3, 2, "ac3", None, "eng")];
    Folder::new("x1", &[(2, &s.comment[..], "x2"), (3, &companion[..], "x3")])
        .streams(&streams)
        .langs(&[(1, "spa")])
        .write(&c.join("disc-x/t0"));
    write(&c.join("disc-x/tmdb.json"), r#"{"original_language": "es"}"#);
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    let snap = read_json_file(&c.join(BASELINE_FILE));
    let cands: Vec<&Value> = snap["tracks"].as_array().unwrap().iter().filter(|t| t["role"] == "candidate").collect();
    let both: Vec<(&Value, &Value)> = cands.iter().map(|t| (&t["verdict"], &t["swapped_verdict"])).collect();
    assert_eq!(both, vec![(&Value::Null, &Value::Null); 2]);
    assert_eq!((&cands[1]["peer"]["stream"], &cands[1]["peer"]["verdict"]), (&json!(2), &Value::Null));
    let mut hidden: Vec<&str> = snap["unlabelled_verdicts"].as_object().unwrap().values().map(|v| v.as_str().unwrap()).collect();
    hidden.sort();
    assert_eq!(hidden, ["commentary-cross-language", "note-commentary"]);
}

#[test]
fn a_folder_for_a_language_that_is_not_wanted_is_dataset_only() {
    let tmp = new_tmp("dataset-only");
    let (s, c) = (signals(), tmp.corpus());
    let streams: [Stream; 3] = [(1, 8, "truehd", None, "eng"), (2, 6, "ac3", None, "fra"), (3, 2, "ac3", None, "fra")];
    Folder::new("f1", &[]).streams(&streams).channels(8).write(&c.join("disc-f/t0"));
    Folder::new("f2", &[(3, &s.copy[..], "f3")]).main(2).streams(&streams).langs(&[(2, "fra"), (3, "fra")]).write(&c.join("disc-f/t0"));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("(0 measured against the wrong reference)"), "{out}");
}

#[test]
fn a_reference_labelled_unwanted_is_its_own_failure_not_a_swapped_one() {
    let tmp = new_tmp("unwanted-not-swapped");
    let (s, c) = (signals(), tmp.corpus());
    let d = Folder::new("v1", &[(2, &s.mix[..], "v2")]).fc(false).write(&c.join("disc-v/t0"));
    fs::write(d.join("main-mix.txt"), level_file_text(&s.comment)).unwrap(); // a commentary ranked first: the programme is the candidate
    write_labels(&c.join("disc-v"), vec![entry("t0", 1, "commentary"), entry("t0", 2, "lossy-companion")], json!({}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL disc-v/t0 stream 1: the reference is labelled commentary (unwanted audio)"), "{out}");
    assert!(!out.contains("swapped reference:"), "{out}");
    let snap = read_json_file(&c.join(BASELINE_FILE));
    let rows: BTreeMap<i64, Value> = snap["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["role"] == "candidate")
        .map(|t| (t["stream"].as_i64().unwrap(), t.clone()))
        .collect();
    assert_eq!((&rows[&2]["verdict"], &rows[&2]["swapped_verdict"]), (&json!("reference-unclear"), &json!("commentary")));
}

#[test]
fn a_vetoed_title_has_its_candidates_reviewed_not_counted_as_commentary_verdicts() {
    // D21: the reference (stream 2, lossless) is not the first eng stream, and stream 1 has six channels
    let tmp = new_tmp("veto");
    let (s, c) = (signals(), tmp.corpus());
    let streams: [Stream; 3] = [(1, 6, "dts", Some("DTS"), "eng"), (2, 6, "dts", Some("DTS-HD MA"), "eng"), (3, 2, "ac3", None, "eng")];
    Folder::new("q2", &[(1, &s.copy[..], "q1"), (3, &s.comment[..], "q3")]).main(2).streams(&streams).write(&c.join("disc-q/t0"));
    write_labels(
        &c.join("disc-q"),
        vec![entry("t0", 1, "lossy-companion"), with(entry("t0", 3, "commentary"), "film_audible", json!(true))],
        json!({"t0": title("feature", false)}),
    );
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("note: disc-q/t0/audio-levels-20ms-2-1-3: the position veto holds (row 0, D21): the reference, stream 2 (dts DTS-HD MA 6 ch t2), is not the first eng stream of the file: stream 1 (dts DTS 6 ch t1) comes first and is lossless or has more than two channels"), "{out}");
    let rows = table_rows(&out);
    assert_eq!(words(&rows["commentary"][2..6]), ["0", "0", "0", "1"]); // reviewed (r-unc), not a commentary verdict
    assert_eq!(words(&rows["lossy-companion"][2..6]), ["0", "0", "0", "1"]); // a keep stays a keep
    assert!(out.contains("commentaries          1 of 8, from 1 of 4 discs"), "{out}"); // the label still counts ...
    assert!(out.contains("commentary verdicts   on 0 of 3 discs"), "{out}"); // ... but not as a verdict
    assert!(out.contains("false verdicts        0"), "{out}");
    assert!(out.contains("kept although unwanted, commentary: disc-q/t0 stream 3 (reference-unclear)"), "{out}");
    let snap = read_json_file(&c.join(BASELINE_FILE));
    let rows: BTreeMap<i64, Value> =
        snap["tracks"].as_array().unwrap().iter().map(|t| (t["stream"].as_i64().unwrap(), t.clone())).collect();
    assert_eq!((&rows[&3]["verdict"], &rows[&3]["before_veto"]), (&json!("reference-unclear"), &json!("commentary")));
    assert!(rows[&2]["veto"].as_str().unwrap().starts_with("the reference, stream 2 (dts DTS-HD MA 6 ch t2), is not the first"));
}

#[test]
fn a_folder_without_streams_json_counts_none_of_its_candidates() {
    // The vetoed title above without its streams.json: the veto cannot be checked, so its commentary is neither a
    // commentary verdict nor a counted commentary, and the report says why.
    let tmp = new_tmp("no-streams");
    let (s, c) = (signals(), tmp.corpus());
    let streams: [Stream; 3] = [(1, 6, "dts", Some("DTS"), "eng"), (2, 6, "dts", Some("DTS-HD MA"), "eng"), (3, 2, "ac3", None, "eng")];
    let d = Folder::new("q2", &[(3, &s.comment[..], "q3")]).main(2).streams(&streams).write(&c.join("disc-q/t0"));
    fs::remove_file(d.join("streams.json")).unwrap();
    write_labels(&c.join("disc-q"), vec![with(entry("t0", 3, "commentary"), "film_audible", json!(true))], json!({"t0": title("feature", false)}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("commentaries          0 of 8, from 0 of 4 discs"), "{out}");
    assert!(out.contains("commentary verdicts   on 0 of 3 discs"), "{out}");
    assert!(out.contains("not counted: disc-q 1"), "{out}");
    assert!(out.contains("note: disc-q/t0/audio-levels-20ms-2-3: no streams.json, so neither its reference nor the position veto (row 0, D21) is checked: none of its candidates counts for experiment 13"), "{out}");
    let snap = read_json_file(&c.join(BASELINE_FILE));
    let rows: BTreeMap<i64, Value> =
        snap["tracks"].as_array().unwrap().iter().map(|t| (t["stream"].as_i64().unwrap(), t.clone())).collect();
    assert_eq!(rows[&3]["why_not_counted"], "the position veto (row 0, D21) is not checked: the folder has no streams.json");
    assert_eq!(rows[&3]["verdict"], "commentary"); // measured and judged by rows 3 to 9, just not counted
}

#[test]
fn a_candidate_the_app_never_judges_is_measured_not_counted() {
    // An English film: a French 5.1 and its 2.0 companion (a dataset-only folder), and a commentary with no language
    // code. The app judges neither language (R4), so neither counts, nor fails, whatever its verdict.
    let tmp = new_tmp("never-judged");
    let (s, c) = (signals(), tmp.corpus());
    let streams: [Stream; 3] = [(1, 8, "truehd", None, "eng"), (2, 6, "ac3", None, "fra"), (3, 2, "ac3", None, "fra")];
    Folder::new("f1", &[]).streams(&streams).channels(8).write(&c.join("disc-f/t0"));
    let fra = Folder::new("f2", &[(3, &s.copy[..], "f3")]).main(2).streams(&streams).langs(&[(2, "fra"), (3, "fra")]).write(&c.join("disc-f/t0"));
    write_labels(&c.join("disc-f"), vec![entry("t0", 3, "lossy-companion")], json!({"t0": title("feature", false)}));
    Folder::new("n1", &[(2, &s.comment[..], "n2")])
        .langs(&[(2, "und")])
        .streams(&[(1, 6, "dts", None, "eng"), (2, 2, "ac3", None, "und")])
        .write(&c.join("disc-n/t0"));
    write_labels(&c.join("disc-n"), vec![with(entry("t0", 2, "commentary"), "film_audible", json!(true))], json!({"t0": title("feature", false)}));
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("programme tracks      0 of 20 other than the reference; separately encoded 0 of 6"), "{out}");
    assert!(out.contains("commentaries          0 of 8, from 0 of 4 discs"), "{out}");
    assert!(out.contains("commentary verdicts   on 0 of 3 discs"), "{out}");
    assert!(out.contains("not counted: disc-f 1, disc-n 1"), "{out}");
    assert!(out.contains("note: disc-f/t0 stream 3: fra is not a wanted language: the app never judges it, measured for the dataset only; labelled lossy-companion, verdict keep: not counted for experiment 13 and never a FAIL"), "{out}");
    assert!(out.contains("note: disc-n/t0 stream 2: no language code (und): the app never judges it, measured for the dataset only; labelled commentary, verdict commentary"), "{out}");
    let rows = table_rows(&out);
    assert!(!rows.contains_key("commentary"), "{out}");
    assert!(!rows.contains_key("lossy-companion"), "{out}");
    assert_eq!(words(&rows["(not"][2..12]), ["0", "1", "0", "0", "0", "0", "0", "1", "0", "2"]); // in (not judged): comm, keep
    // The reverse case: the commentary pattern on the French companion is no FAIL and no false verdict.
    fs::write(fra.join("cand-3.txt"), level_file_text(&s.comment)).unwrap();
    let t = c.join("t.json").display().to_string();
    let (code, out) = retune(&tmp, &["--snapshot", t.as_str()]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("  FAIL "), "{out}");
    assert!(out.trim_end().ends_with("RESULT ok"), "{out}");
    assert!(out.contains("false verdicts        0"), "{out}");
    assert!(out.contains("labelled lossy-companion, verdict commentary: not counted"), "{out}");
    assert!(out.contains("as it stands: not answered"), "{out}"); // not "no: 1 programme track(s) with the commentary pattern"
}

// ---------------------------------------------------------------------------------------------------------------
// New: the Python's lines, Review Focus 2, 3 and 5, the cache, a v2 baseline, the arguments, the command table

#[test]
fn the_main_corpus_report_is_the_pythons_line_for_line() {
    let run = main_run();
    assert_eq!(run.out, MAIN_REPORT);
}

#[test]
fn a_second_run_of_the_main_corpus_is_the_pythons_line_for_line() {
    let tmp = baseline_corpus("second-lines");
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    let expected = MAIN_REPORT
        .replace("measures: 2 analysed now, 2 taken from the cache", "measures: 0 analysed now, 4 taken from the cache")
        .replace(
            "\nsnapshot: corpus/owner/audio-dataset.json\n",
            "\nChanges since the previous snapshot (2026-10-13T12:00:00, rule version 1):\n  none\n\nsnapshot: corpus/owner/audio-dataset.json (unchanged but for its time: not rewritten)\n",
        );
    assert_eq!(out, expected);
}

#[test]
fn two_level_folders_of_one_label_and_a_dataset_only_title() {
    // Review Focus 3, as pans-labyrinth-2006-uhd has them: all-tracks measured in 1-2 and in 1-2-3, and eng-only
    // measured for the dataset only (its main stream, no candidate). The lines are the Python's for these files.
    let tmp = new_tmp("two-folders");
    let (s, c) = (signals(), tmp.corpus());
    Folder::new("p1", &[(2, &s.copy[..], "p2")]).write(&c.join("disc-p/all-tracks"));
    Folder::new("p1", &[(2, &s.copy[..], "p2"), (3, &s.comment[..], "p3")]).write(&c.join("disc-p/all-tracks"));
    write_labels(
        &c.join("disc-p"),
        vec![entry("all-tracks", 2, "lossy-companion"), with(entry("all-tracks", 3, "commentary"), "film_audible", json!(true))],
        json!({"all-tracks": title("feature", false)}),
    );
    let without_path = tmp.path("without.json");
    let (code, without) = retune(&tmp, &["--snapshot", without_path.as_str()]);
    assert_eq!(code, 0, "{without}");
    Folder::new("p1", &[]).write(&c.join("disc-p/eng-only"));
    let with_path = tmp.path("with.json");
    let (code, with_dataset_only) = retune(&tmp, &["--snapshot", with_path.as_str()]);
    assert_eq!(code, 0, "{with_dataset_only}");
    for out in [&without, &with_dataset_only] {
        // each folder judged, the table from the folder with the most candidates
        assert!(out.contains("  note: disc-p/all-tracks stream 1 (reference) is measured in audio-levels-20ms-1-2, audio-levels-20ms-1-2-3; the table uses audio-levels-20ms-1-2-3\n"), "{out}");
        assert!(out.contains("  note: disc-p/all-tracks stream 2 (candidate) is measured in audio-levels-20ms-1-2, audio-levels-20ms-1-2-3; the table uses audio-levels-20ms-1-2-3\n"), "{out}");
        let rows = table_rows(out);
        assert_eq!((rows["lossy-companion"].last().unwrap().as_str(), rows["commentary"].last().unwrap().as_str()), ("1", "1"));
        assert!(out.contains("  commentaries          1 of 8, from 1 of 4 discs; on feature films 1 of 2; film clearly audible 1 of 1 (of them partial: 0)\n"), "{out}");
        assert!(out.contains("  programme tracks      1 of 20 other than the reference; separately encoded 1 of 6; derived cores 0 (row 13 does not exclude them from the 20)\n"), "{out}");
        assert!(out.contains("  not counted: none\n"), "{out}");
    }
    // the dataset-only folder adds a title, a level folder and a reference, and nothing to the counts
    assert!(without.contains("audio dataset: corpus/owner: 1 disc, 1 title, 2 level folders (0 measured against the wrong reference), 2 measured tracks (2 labelled), 1 reference\n"), "{without}");
    assert!(with_dataset_only.contains("audio dataset: corpus/owner: 1 disc, 2 titles, 3 level folders (0 measured against the wrong reference), 2 measured tracks (2 labelled), 2 references\n"), "{with_dataset_only}");
    assert_eq!(section(&with_dataset_only, "Experiment 13 ("), section(&without, "Experiment 13 ("));
    assert!(with_dataset_only.contains("0 of 2 (every successful rip and every measured title); missing: disc-p/all-tracks, disc-p/eng-only"), "{with_dataset_only}");
    let snap = read_json_file(Path::new(&with_path));
    let got: Vec<(String, String, String, i64, bool, Value, bool)> = snap["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (
            t["label"].as_str().unwrap().to_string(),
            t["folder"].as_str().unwrap().to_string(),
            t["role"].as_str().unwrap().to_string(),
            t["stream"].as_i64().unwrap(),
            t["secondary"].as_bool().unwrap(),
            t["verdict"].clone(),
            t["counts"].as_bool().unwrap(),
        ))
        .collect();
    let row = |l: &str, f: &str, r: &str, n: i64, sec: bool, v: Value, counts: bool| (l.to_string(), f.to_string(), r.to_string(), n, sec, v, counts);
    assert_eq!(got, vec![
        row("all-tracks", "audio-levels-20ms-1-2", "reference", 1, true, Value::Null, false),
        row("all-tracks", "audio-levels-20ms-1-2", "candidate", 2, true, json!("keep"), false),
        row("all-tracks", "audio-levels-20ms-1-2-3", "reference", 1, false, Value::Null, false),
        row("all-tracks", "audio-levels-20ms-1-2-3", "candidate", 2, false, json!("keep"), true),
        row("all-tracks", "audio-levels-20ms-1-2-3", "candidate", 3, false, json!("commentary"), true),
        row("eng-only", "audio-levels-20ms-1", "reference", 1, false, Value::Null, false),
    ]);
}

#[test]
fn golden_folders_are_never_read() {
    // Review Focus 2: spec section 5 reserves golden/. A level folder, a rip summary and a labels file that would
    // stop the run, all under golden/ folders, change nothing: the run is the same as without them.
    let plain = new_tmp("plain");
    build_main_corpus(&plain.corpus());
    let golden = new_tmp("golden");
    build_main_corpus(&golden.corpus());
    let (s, c) = (signals(), golden.corpus());
    Folder::new("mb", &[(3, &s.comment[..], "cb")]).write(&c.join("disc-b/golden"));
    write(&c.join("disc-b/golden/rip-summary.json"), r#"{"ok": true, "title": 0, "bytes": 100}"#);
    Folder::new("g1", &[(2, &s.copy[..], "g2")]).write(&c.join("golden/t0"));
    write(&c.join("golden/t0/rip-summary.json"), r#"{"ok": true}"#);
    write(&c.join("golden/audio-labels.json"), "not JSON: read, it would stop the run with exit 2");
    let (code, out) = retune(&plain, &[]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(retune(&golden, &[]), (code, out));
}

#[test]
fn the_cache_gives_what_a_run_without_it_gives_and_a_stale_one_is_seen() {
    // Spec 3.3: a test runs the harness with the cache off and on and compares, so a missed RULE_VERSION bump is
    // caught. Here the cache is this run's own baseline, then one doctored as a stale cache would be.
    let tmp = baseline_corpus("cache");
    let text = fs::read_to_string(tmp.corpus().join(BASELINE_FILE)).unwrap();
    let previous = read_snapshot(&text).expect("the baseline is a v3 snapshot");
    assert_eq!(previous.measures_cache.len(), 2);
    let tree = DiskTree::open(&tmp.corpus(), EVIDENCE_SHOWN).unwrap();
    let req = exp13_requirements(&spec_text(&[])).unwrap();
    let overrides = Overrides::default();
    let harness_with = |previous: &PreviousSnapshot, use_cache: bool| {
        harness(&HarnessInput {
            tree: &tree,
            req: &req,
            overrides: &overrides,
            previous: Some(previous),
            compare: None,
            use_cache,
            created: "2026-10-13T12:00:00",
        })
        .unwrap()
    };
    let on = harness_with(&previous, true);
    let off = harness_with(&previous, false);
    assert_eq!((on.computed, on.reused), (0, 4));
    assert_eq!((off.computed, off.reused), (4, 0));
    assert_eq!(value_of(&on), value_of(&off));
    assert_eq!(strip_measures(&on.lines), strip_measures(&off.lines));
    // A measure changed without RULE_VERSION being bumped: the cache keeps the old value under the same key, so
    // only the run without the cache sees the new one.
    let key = previous
        .tracks
        .iter()
        .find(|t| t["slug"] == "disc-b" && t["label"] == "all-tracks" && t["stream"] == 3)
        .and_then(|t| t["key"].as_str())
        .unwrap()
        .to_string();
    let mut stale = previous.clone();
    match stale.measures_cache.get_mut(&key) {
        Some(Measures::Run(run)) => run.dc = 0.0,
        other => panic!("the commentary's measures: {other:?}"),
    }
    assert_ne!(value_of(&harness_with(&stale, true)), value_of(&off));
    assert_eq!(value_of(&harness_with(&stale, false)), value_of(&off));
}

#[test]
fn a_v2_baseline_is_read_for_its_changes_and_never_as_a_cache() {
    let tmp = baseline_corpus("v2");
    let base = tmp.corpus().join(BASELINE_FILE);
    let mut data = read_json_file(&base);
    data["format"] = json!(SNAPSHOT_FORMAT_V2);
    data["code"] = json!({"audio_similarity_sha1": "3b6278d937d90000000000000000000000000000"});
    let v2_text = dumps(&data, Style::INDENT1_ASCII) + "\n";
    fs::write(&base, &v2_text).unwrap();
    let (code, out) = retune(&tmp, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("measures: 2 analysed now, 2 taken from the cache"), "{out}"); // as on an empty cache
    assert!(out.contains("\nChanges since the previous snapshot (2026-10-13T12:00:00, code 3b6278d937d9):\n  none\n"), "{out}");
    assert!(out.contains("snapshot: corpus/owner/audio-dataset.json\n"), "{out}"); // rewritten as v3
    let now = read_json_file(&base);
    assert_eq!((&now["format"], &now["code"]), (&json!(SNAPSHOT_FORMAT), &json!({"rule_version": 1})));
    let v2 = tmp.path("v2.json");
    fs::write(&v2, &v2_text).unwrap();
    let (code, out) = retune(&tmp, &["--compare", v2.as_str()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains(&format!("Changes against {v2} (2026-10-13T12:00:00, code 3b6278d937d9):\n  none\n")), "{out}");
}

#[test]
fn arguments_the_command_refuses() {
    let tmp = new_tmp("arguments");
    fs::create_dir_all(tmp.corpus()).unwrap();
    let (corpus, spec) = (tmp.path("corpus"), tmp.path("spec.md"));
    // the Python's positional corpus is --evidence now
    let (code, _, err) = call(&tmp, &["--spec", spec.as_str(), "corpus/owner"]);
    assert_eq!(code, 2);
    assert!(err.contains("lazymkv-dev retune: error: unrecognized arguments: corpus/owner"), "{err}");
    let nope = tmp.path("nope.md");
    assert_eq!(call(&tmp, &["--evidence", corpus.as_str(), "--spec", nope.as_str()]),
               (2, String::new(), format!("[Errno 2] No such file or directory: {}\n", py_repr_str(&nope))));
    fs::write(tmp.0.join("bad.md"), "# spec\n").unwrap();
    let bad = tmp.path("bad.md");
    assert_eq!(call(&tmp, &["--evidence", corpus.as_str(), "--spec", bad.as_str()]), (2, String::new(), "spec section 17 has no row starting '| 13 |'\n".to_string()));
    // --set is read before the evidence, as the Python
    let missing = tmp.path("missing");
    assert_eq!(call(&tmp, &["--evidence", missing.as_str(), "--spec", spec.as_str(), "--set", "x"]), (2, String::new(), "--set x: write NAME=VALUE\n".to_string()));
    fs::write(tmp.0.join("t.json"), "{}").unwrap();
    let t = tmp.path("t.json");
    assert_eq!(call(&tmp, &["--evidence", corpus.as_str(), "--spec", spec.as_str(), "--compare", t.as_str()]), (2, String::new(), format!(
        "{t}: not a snapshot written by lazymkv-dev retune (lazymkv audio dataset snapshot v3) or by audio_dataset.py (lazymkv audio dataset snapshot v2)\n")));
}

#[test]
fn retune_is_in_the_command_table_before_levels_args() {
    let names: Vec<&str> = crate::COMMANDS.iter().map(|(name, _, _)| *name).collect();
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap_or_else(|| panic!("{name} in {names:?}"));
    assert!(at("reconcile") < at("retune") && at("retune") < at("levels-args") && at("levels-args") < at("titles"), "{names:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// The evidence clone (each skips with its reason when its input is absent)

/// (repository root, evidence clone): LAZYMKV_EVIDENCE, else corpus/owner under the repository; None, after the
/// skip line, when the clone is absent.
fn evidence() -> Option<(PathBuf, PathBuf)> {
    let repo = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the workspace root");
    let dir = match std::env::var("LAZYMKV_EVIDENCE") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => repo.join("corpus/owner"),
    };
    if !dir.is_dir() {
        skip(&format!("no evidence clone at {}", dir.display()));
        return None;
    }
    Some((repo, dir))
}

/// (slug, label, folder, has streams.json) of every level folder on disk, golden/ and non-slugs left out, in name
/// order.
fn level_folders_on_disk(evidence: &Path) -> Vec<(String, String, String, bool)> {
    fn name(p: &Path) -> String {
        p.file_name().unwrap().to_string_lossy().into_owned()
    }
    let mut out = Vec::new();
    for slug in subdirs(evidence) {
        let s = name(&slug);
        if s == "golden" || !is_slug(&s) {
            continue;
        }
        for label in subdirs(&slug) {
            let l = name(&label);
            if l == "golden" {
                continue;
            }
            for folder in subdirs(&label) {
                let f = name(&folder);
                if f.starts_with("audio-levels-20ms-") {
                    let streams = folder.join("streams.json").is_file();
                    out.push((s.clone(), l.clone(), f, streams));
                }
            }
        }
    }
    out
}

/// The evidence clone as a fresh clone has it: every file but the ones git ignores (*/*/audio-levels-*/*.txt), read
/// lazily from disk, .git left out. Nothing is copied or written.
fn fresh_clone_tree(root: &Path) -> MemTree {
    fn walk(tree: &mut MemTree, root: &Path, rel: &str) {
        let dir = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
        let mut entries: Vec<fs::DirEntry> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap()).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            if rel.is_empty() && name == ".git" {
                continue;
            }
            let child = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            let kind = e.file_type().unwrap();
            if kind.is_dir() {
                tree.insert_dir(&child);
                walk(tree, root, &child);
            } else if kind.is_file() {
                let parts: Vec<&str> = child.split('/').collect();
                let ignored = parts.len() == 4 && parts[2].starts_with("audio-levels-") && parts[3].ends_with(".txt");
                if !ignored {
                    let path = e.path();
                    tree.insert_lazy(&child, Arc::new(move || fs::read(&path).map_err(|e| e.to_string())));
                }
            }
        }
    }
    let mut tree = MemTree::new(EVIDENCE_SHOWN, "owner");
    walk(&mut tree, root, "");
    tree
}

fn design_spec(repo: &Path) -> lazymkv_core::audio::labels::Requirements {
    let bytes = fs::read(repo.join(DESIGN_SPEC)).unwrap();
    exp13_requirements(&decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict).unwrap()).unwrap()
}

fn baseline_of(evidence: &Path) -> Option<PreviousSnapshot> {
    let bytes = fs::read(evidence.join(BASELINE_FILE)).ok()?;
    read_snapshot(&decode_text(&bytes, Encoding::Utf8, Errors::Strict).ok()?)
}

#[test]
fn a_fresh_clone_of_the_evidence_exits_3_with_the_pythons_line() {
    // Review Focus 5: the evidence clone without its level series (git keeps only streams.json): every level folder
    // is a folder that cannot be read, with the Python's line, and the run ends exit 3 without replacing the
    // committed baseline.
    let Some((repo, dir)) = evidence() else { return };
    let folders = level_folders_on_disk(&dir);
    if folders.is_empty() {
        skip(&format!("no level folder under {}", dir.display()));
        return;
    }
    let tree = fresh_clone_tree(&dir);
    let req = design_spec(&repo);
    let previous = baseline_of(&dir);
    let overrides = Overrides::default();
    let h = harness(&HarnessInput {
        tree: &tree,
        req: &req,
        overrides: &overrides,
        previous: previous.as_ref(),
        compare: None,
        use_cache: true,
        created: "2026-10-13T12:00:00",
    })
    .expect("the clone's labels validate");
    let block = section(&(h.lines.join("\n") + "\n\n"), "Folders that cannot be read:");
    for (slug, label, folder, streams) in &folders {
        if *streams {
            let line = format!("  {slug}/{label}/{folder}: {LEVELS_NOT_HERE}");
            assert!(block.lines().any(|l| l == line), "{line}\nnot in:\n{block}");
        }
    }
    assert_eq!(h.unchecked[0], format!("{} folder(s) cannot be read", folders.len()));
    assert!(h.failures.is_empty(), "{:?}", h.failures);
    let (result, code) = result_line(&h.failures, &h.unchecked, false);
    assert_eq!(code, 3, "{result}");
    assert!(result.starts_with(&format!("RESULT FAILED: not everything was checked: {} folder(s) cannot be read", folders.len())), "{result}");
    assert_eq!(destination(false, false, &h.unchecked), Destination::Unchecked); // the committed baseline stays
}

#[test]
fn the_evidence_cache_gives_what_a_run_without_it_gives() {
    // Spec 3.3: the committed v3 baseline's measures, reused, must be what the rule computes now; a difference means
    // a measure, a constant's default or a threshold changed without RULE_VERSION being bumped.
    let Some((repo, dir)) = evidence() else { return };
    let Some(previous) = baseline_of(&dir).filter(|p| p.format == SNAPSHOT_FORMAT) else {
        skip("corpus/owner/audio-dataset.json is not a v3 snapshot yet: a v2 file is never a cache, so there is nothing to compare");
        return;
    };
    let folders = level_folders_on_disk(&dir);
    if folders.iter().any(|(s, l, f, _)| !dir.join(s).join(l).join(f).join("manifest.txt").is_file()) {
        skip("its level files are not here (git keeps only streams.json): pull the recording to compare the cache");
        return;
    }
    let tree = DiskTree::open(&dir, EVIDENCE_SHOWN).unwrap();
    let req = design_spec(&repo);
    let overrides = Overrides::default();
    let harness_with = |use_cache: bool| {
        harness(&HarnessInput {
            tree: &tree,
            req: &req,
            overrides: &overrides,
            previous: Some(&previous),
            compare: None,
            use_cache,
            created: "2026-10-13T12:00:00",
        })
        .unwrap()
    };
    let on = harness_with(true);
    let off = harness_with(false);
    assert_eq!(off.reused, 0);
    let exact = Tolerance { abs: 0.0, loose: 0.0, loose_keys: &[], ignore: Vec::new() };
    let diff = structural_diff(&value_of(&off), &value_of(&on), &exact);
    assert!(
        diff.is_empty(),
        "the cached measures are not what the rule computes now: bump core::audio::rule::RULE_VERSION\n{}",
        diff.join("\n")
    );
    assert_eq!(strip_measures(&on.lines), strip_measures(&off.lines));
}


// ---------------------------------------------------------------------------------------------------------------
// Counts (handed over by Task 7): every counting rule of experiment 13 decides one of the numbers the report prints

/// The Python test's build_count_corpus(c): every counting rule of experiment 13 decides one of the numbers this
/// corpus prints.
fn build_count_corpus(c: &Path) {
    let s = signals();
    let (comment, copy) = (&s.comment[..], &s.copy[..]);
    Folder::new(
        "d1",
        &[(2, comment, "d2"), (3, comment, "d3"), (4, copy, "d4"), (5, copy, "d5"), (6, copy, "d6"), (7, copy, "d7"), (8, comment, "d8")],
    )
    .write(&c.join("disc-d/t0"));
    Folder::new("d1", &[(2, comment, "d2")]).write(&c.join("disc-d/t1")); // the same commentary again, labelled again: counts once
    // Streams 7 and 8 are named from the packaging and from TheDiscDB only: observations, never counted (D23).
    write_labels(
        &c.join("disc-d"),
        vec![
            with(entry("t0", 2, "commentary"), "film_audible", json!(true)),
            with(entry("t0", 3, "commentary"), "film_audible", Value::Null),
            entry("t0", 4, "lossy-companion"),
            entry("t0", 5, "derived-core"),
            entry("t0", 6, "main"),
            with(entry("t0", 7, "lossy-companion"), "how", json!("packaging")),
            with(with(entry("t0", 8, "commentary"), "how", json!("thediscdb")), "film_audible", json!(true)),
            entry("t1", 2, "commentary"),
        ],
        json!({"t0": title("feature", true), "t1": title("feature", true)}),
    );
    Folder::new("e1", &[(2, comment, "e2"), (3, comment, "e3"), (4, comment, "e4"), (5, comment, "e5")])
        .fc(false)
        .channels(2)
        .write(&c.join("disc-e/t0"));
    write_labels(
        &c.join("disc-e"),
        vec![
            with(entry("t0", 2, "commentary"), "film_audible", json!(false)),
            with(entry("t0", 3, "commentary"), "saw_verdict", json!(true)),
            with(entry("t0", 4, "commentary"), "sure", json!(false)),
            with(entry("t0", 5, "commentary"), "how", json!("inferred")),
        ],
        json!({"t0": title("episode", false)}),
    );
    Folder::new("g1", &[(2, comment, "g2")]).write(&c.join("disc-g/t0"));
    write_labels(&c.join("disc-g"), vec![entry("t0", 2, "commentary")], json!({"t0": title("feature", true)}));
    Folder::new("h1", &[(2, comment, "h2"), (3, comment, "h3")]).fc(false).channels(2).write(&c.join("disc-h/t0"));
    Folder::new("h1", &[(2, comment, "h2")]).fc(false).channels(2).write(&c.join("disc-h/t0b")); // the fitted track again, labelled again
    write_labels(
        &c.join("disc-h"),
        vec![entry("t0", 2, "commentary"), entry("t0", 3, "commentary"), entry("t0b", 2, "commentary")],
        json!({"t0": title("episode", true)}),
    );
    Folder::new("k1", &[(2, copy, "k2")]).write(&c.join("disc-k/t0")); // a commentary the rule misses: its disc has no commentary verdict
    write_labels(&c.join("disc-k"), vec![entry("t0", 2, "commentary")], json!({"t0": title("episode", false)}));
    // labels.add_fitted(c, "disc-g", None, None, "test", "", "2026-10-05") and
    // labels.add_fitted(c, "disc-h", "t0", 2, "test", "", "2026-10-05"): one file, the two entries in that order.
    let shown = "corpus/owner/audio-fitted.json";
    let (_, fitted) = lazymkv_core::audio::labels::add_fitted(None, shown, "disc-g", None, None, "test", "", "2026-10-05").unwrap();
    let (_, fitted) =
        lazymkv_core::audio::labels::add_fitted(Some(&fitted), shown, "disc-h", Some("t0"), Some(2), "test", "", "2026-10-05").unwrap();
    crate::pyjson::write_labels(&c.join("audio-fitted.json"), &fitted).unwrap();
}

/// The count corpus after its first run (spec.md, the default row 13), with short.md and met.md beside spec.md.
/// Py: Counts.setUpClass.
struct CountRun {
    tmp: TempDir,
    code: i32,
    out: String,
}

fn count_run(tag: &str) -> CountRun {
    let tmp = new_tmp(tag);
    build_count_corpus(&tmp.corpus());
    // spec(comm=4, discs=3, prog=2, sep=1, centre=2, heavy=2, verdict_discs=4) and the same with verdict_discs=3
    let short: [(&str, i64); 7] =
        [("comm", 4), ("discs", 3), ("prog", 2), ("sep", 1), ("centre", 2), ("heavy", 2), ("verdict_discs", 4)];
    let met: [(&str, i64); 7] =
        [("comm", 4), ("discs", 3), ("prog", 2), ("sep", 1), ("centre", 2), ("heavy", 2), ("verdict_discs", 3)];
    fs::write(tmp.0.join("short.md"), spec_text(&short)).unwrap();
    fs::write(tmp.0.join("met.md"), spec_text(&met)).unwrap();
    let (code, out) = retune(&tmp, &[]); // its snapshot is the cache of the runs below
    CountRun { tmp, code, out }
}

#[test]
fn counts_each_rule_decides_its_number() {
    let run = count_run("counts-rules");
    assert_eq!(run.code, 0, "{}", run.out);
    let e = section(&run.out, "Experiment 13 (");
    assert!(e.contains("commentaries          5 of 8, from 4 of 4 discs; on feature films 2 of 2; film clearly audible 1 of 1"), "{e}");
    assert!(e.contains("programme tracks      2 of 20 other than the reference; separately encoded 1 of 6; derived cores 1"), "{e}");
    assert!(e.contains("no centre channel     2 of 1 titles"), "{e}");
    assert!(e.contains("dialogue-heavy        2 of 1 titles"), "{e}");
    assert!(e.contains("commentary verdicts   on 3 of 3 discs"), "{e}");
    assert!(e.contains("not counted: disc-d 4, disc-e 3, disc-g 1, disc-h 2"), "{e}"); // disc-d: main, t1's copy, two observations
}

#[test]
fn counts_the_answer_follows_the_rules_of_row_13() {
    let run = count_run("counts-answer");
    assert!(
        run.out.contains("as it stands: not answered: too few commentaries, programme tracks, separately encoded programme tracks"),
        "{}",
        run.out
    );
    let corpus = run.tmp.path("corpus");
    for (spec, answer) in [
        ("short.md", "as it stands: no: commentary verdicts on 3 discs, 4 needed"),
        ("met.md", "as it stands: yes, if the audio-description"),
    ] {
        let spec = run.tmp.path(spec);
        let (_, out, err) = call(&run.tmp, &["--evidence", corpus.as_str(), "--spec", spec.as_str()]);
        let out = out + &err;
        assert!(out.contains(answer), "{spec}: {out}");
    }
}
