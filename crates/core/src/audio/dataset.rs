//! The tuning harness of the audio rule (decision D16; spec section 15's `retune`) and the job lines that add a
//! ripped title to it (`levels-args`): every level folder of the evidence measured with the rule in force, joined
//! with the owner's labels, counted against row 13 of the spec's section 17, the FAIL rules, the snapshot (format v3:
//! `code` is the rule version, the measures cache is keyed by spec 3.3's formula), and the change blocks.
//! Ported from scripts/phase0/audio_dataset.py in its loop order. The report lines are the Python's byte for byte
//! except spec 3.4's five rewrites (the To-label hint names `lazymkv-dev labels set`, the wrong-reference FAIL line
//! names `lazymkv-dev levels-args`, the streams.json note names `lazymkv-dev levels-from-proxy`, the `rule:` line
//! gives the rule version, and a change block gives `rule version <n>` for a v3 snapshot and keeps `code <sha>` for a
//! v2 one); every path is printed under `corpus/owner` whatever folder the tree was read from. No I/O: the caller
//! hands in the tree, the spec's row 13 and the snapshots read back, and writes the snapshot returned.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::audio::labels::{
    is_fitted, is_slug, kind_of, load, load_fitted, not_counted_because, Fitted, FittedKey, LabelEntry, Labels,
    Requirements, TitleFacts, COMMENTARY_KINDS, KINDS,
};
use crate::audio::measures::{analyse, swapped, Constants, Measures, Num};
use crate::audio::reference::{
    ffprobe_audio, first_of_language, original_language, placed_first, programme_reference, read_streams, wanted,
    AudioStream,
};
use crate::audio::rule::{
    first_of_language as rule_first_of_language, results, verdict, AnalyseFn, Measured, Peer, Rule, Verdict,
    RULE_VERSION,
};
use crate::lang::NO_LANGUAGE;
use crate::recording::files::{FfprobeStream, FfprobeStreams, MkvmergeIdent};
use crate::recording::{
    py_float, py_int, py_int_value, py_repr, py_repr_str, py_str, py_truthy, read_json, Ordered, PathKey, Tree,
};

/// The snapshot written from now on, and the one still read (for --compare and the "Changes since" block only,
/// never as a cache seed). Py: SNAPSHOT_FORMAT.
pub const SNAPSHOT_FORMAT: &str = "lazymkv audio dataset snapshot v3";
pub const SNAPSHOT_FORMAT_V2: &str = "lazymkv audio dataset snapshot v2";
/// The table's column names, matching Verdict::ALL. Py: SHORT.
pub const SHORT: [&str; 9] = ["dup", "comm", "c-x", "r-unc", "ad-p", "n-com", "n-ad", "keep", "n-j"];
/// The verdicts that drop a track: by MD5 (D13), by content once experiment 13 says yes. Py: DROP.
pub const DROP: [Verdict; 2] = [Verdict::Duplicate, Verdict::Commentary];
/// The commentary pattern confirmed: a drop verdict in the reference's language, keep-only in another (addendum 9.2).
/// On a programme track either one is a false verdict. Py: PATTERN.
pub const PATTERN: [Verdict; 2] = [Verdict::Commentary, Verdict::CommentaryCrossLanguage];
/// The file names the harness writes: the committed baseline in the evidence, the trial and the unchecked snapshots
/// in the temporary folder.
pub const BASELINE_FILE: &str = "audio-dataset.json";
pub const TRIAL_FILE: &str = "lazymkv-audio-dataset-trial.json";
pub const UNCHECKED_FILE: &str = "lazymkv-audio-dataset-unchecked.json";
/// Why a level folder git keeps only the streams.json of cannot be read, after "<slug>/<label>/<folder>: ".
/// Py: measure's unreadable line.
pub const LEVELS_NOT_HERE: &str =
    "its level files are not here (git keeps only streams.json): pull the recording or copy the folder from the dataset share";

/// The evidence root as every report line and the snapshot's "corpus" print it (spec 3.4).
const CORPUS: &str = "corpus/owner";
/// The folder name spec section 5 reserves for goldens; the tree's list and glob never return it.
const GOLDEN: &str = "golden";
/// levels_args' refusal of a golden/ folder named as the label (Review Focus 2; the Python had no such case).
const GOLDEN_NOT_A_RIP: &str = "golden/ holds the goldens of spec section 5, never a rip";
/// The last line when a labels or fitted file does not validate (Open point 13: the Python named audio_labels.py check).
const LABELS_INVALID: &str = "RESULT FAILED: a labels file does not validate (lazymkv-dev labels check)";
const UNLABELLED_NOTE: &str =
    "verdicts of tracks the owner has not labelled: never show them to the owner before the label is given";
const HINT: &str = " (see 'Folders that cannot be read', 'Labels without a measurement' and the changes above)";
const OTHER_KINDS: [&str; 4] = ["isolated-score", "music-effects", "same-language-dub", "alternate-narration"];
const YES: &str = "yes, if the audio-description and 'every one the owner has' kinds are covered (checked by hand); for candidates in the reference's language only: one in another language is never dropped by content (addendum 9.2)";

// ---------------------------------------------------------------------------------------------------------------
// --set, the cache

/// The --set values: rule keys (lower case) and constants (upper case), each in the order first given.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    pub rule: Ordered<Num>,
    pub constants: Ordered<Num>,
}

impl Overrides {
    pub fn is_empty(&self) -> bool {
        self.rule.is_empty() && self.constants.is_empty()
    }

    /// {**rule, **constants}: the keys the report names (upper-cased when printed).
    pub fn shown(&self) -> Ordered<Num> {
        let mut out = Ordered::new();
        for (key, value) in self.rule.iter().chain(self.constants.iter()) {
            out.insert(key.to_string(), *value);
        }
        out
    }

    /// Rule::default() and Constants::default() with the overrides applied.
    pub fn effective(&self) -> (Rule, Constants) {
        let mut rule = Rule::default();
        for (key, value) in self.rule.iter() {
            rule.set(key, value.as_f64()).expect("parse_sets checked every rule key and value");
        }
        let mut consts = Constants::default();
        for (key, value) in self.constants.iter() {
            consts.set(key, value.as_f64()).expect("parse_sets checked every constant and value");
        }
        (rule, consts)
    }
}

/// NAME=VALUE items to Overrides with the Python's messages; a name is tried lower-cased against the rule's keys,
/// then upper-cased against the constants; a whole-number key takes a whole value only. Py: parse_sets.
pub fn parse_sets(items: &[String]) -> Result<Overrides, String> {
    let mut out = Overrides::default();
    for item in items {
        let Some((name, text)) = item.split_once('=') else {
            return Err(format!("--set {item}: write NAME=VALUE"));
        };
        let Some(value) = py_float(text) else {
            return Err(format!("--set {item}: {} is not a number", py_repr_str(text)));
        };
        if !value.is_finite() {
            return Err(format!("--set {item}: not a finite number"));
        }
        let lower = name.to_lowercase();
        let upper = name.to_uppercase();
        if Rule::NAMES.iter().any(|(key, _)| *key == lower) {
            // Rule::set gives the value as the Python stored it: an int for tenths_min, a float otherwise.
            let Ok(num) = Rule::default().set(&lower, value) else {
                return Err(format!("--set {item}: {lower} takes a whole number"));
            };
            out.rule.insert(lower, num);
        } else if Constants::NAMES.iter().any(|(key, _)| *key == upper) {
            let Ok(num) = Constants::default().set(&upper, value) else {
                return Err(format!("--set {item}: {upper} takes a whole number"));
            };
            out.constants.insert(upper, num);
        } else {
            let rule_names: Vec<String> = Rule::NAMES.iter().map(|(key, _)| key.to_uppercase()).collect();
            let const_names: Vec<&str> = Constants::NAMES.iter().map(|(key, _)| *key).collect();
            return Err(format!(
                "--set {name}: neither a rule threshold ({}) nor a measure constant ({})",
                rule_names.join(", "),
                const_names.join(", ")
            ));
        }
    }
    Ok(out)
}

/// "lazymkv-rule:" + RULE_VERSION + consts.canonical_json(): the part of every cache key fixed for a run (RP5;
/// replaces the Python's SHA-1 of audio_similarity.py).
pub fn cache_prefix(consts: &Constants) -> String {
    format!("lazymkv-rule:{RULE_VERSION}{}", consts.canonical_json())
}

/// sha1(prefix, then for main, cand and dialogue in turn: the series' little-endian f64 bytes and "|", or "|" alone
/// for None) as 40 lowercase hex digits. Py: Measurer.__call__'s key, `array.array("d", series).tobytes()`.
pub fn cache_key(prefix: &str, main: &[f64], cand: &[f64], dialogue: Option<&[f64]>) -> String {
    let mut h = Sha1::new();
    h.update(prefix.as_bytes());
    for series in [Some(main), Some(cand), dialogue] {
        if let Some(values) = series {
            let mut bytes = Vec::with_capacity(values.len() * 8);
            for v in values {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            h.update(&bytes);
        }
        h.update(b"|");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// analyse with a cache keyed by cache_key: a key in `cache` is reused (counted in `reused`), any other is computed
/// (counted in `computed`) and added to `cache`, so the same inputs again in this run (a re-ripped title) are not
/// analysed twice; every key used is recorded in `used`. With `use_cache` false nothing is reused (the test that
/// catches a missed RULE_VERSION bump). Py: Measurer.
pub struct Measurer<'a> {
    pub cache: &'a mut BTreeMap<String, Measures>,
    pub used: BTreeMap<String, Measures>,
    pub computed: usize,
    pub reused: usize,
    consts: &'a Constants,
    prefix: String,
    use_cache: bool,
}

impl<'a> Measurer<'a> {
    pub fn new(cache: &'a mut BTreeMap<String, Measures>, consts: &'a Constants, use_cache: bool) -> Measurer<'a> {
        Measurer {
            cache,
            used: BTreeMap::new(),
            computed: 0,
            reused: 0,
            consts,
            prefix: cache_prefix(consts),
            use_cache,
        }
    }
}

impl AnalyseFn for Measurer<'_> {
    /// An Err of analyse is passed on and nothing is cached, counted or marked used, as the Python's Measurer did
    /// when sim.analyse raised.
    fn measure(&mut self, main: &[f64], cand: &[f64], dialogue: Option<&[f64]>) -> Result<Measured, String> {
        let key = cache_key(&self.prefix, main, cand, dialogue);
        let cached = if self.use_cache { self.cache.get(&key).cloned() } else { None };
        let measures = match cached {
            Some(m) => {
                self.reused += 1;
                m
            }
            None => {
                let m = analyse(main, cand, dialogue, true, self.consts)?;
                self.computed += 1;
                self.cache.insert(key.clone(), m.clone());
                m
            }
        };
        self.used.insert(key.clone(), measures.clone());
        Ok(Measured { measures, key: Some(key) })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The level folders, and the jobs that add a title

/// (slug, label, folder) of every audio-levels-20ms-* folder: slug folders matching is_slug, label folders, folder
/// names, each in PathKey order, `golden` never included (the tree leaves it out). Py: find_folders.
pub fn find_folders(tree: &dyn Tree) -> Vec<(String, String, PathKey)> {
    let mut out = Vec::new();
    for slug_dir in tree.list(&PathKey::root()) {
        if !tree.is_dir(&slug_dir) || !is_slug(slug_dir.name()) {
            continue;
        }
        for label_dir in tree.list(&slug_dir) {
            if !tree.is_dir(&label_dir) {
                continue;
            }
            for folder in tree.glob(&label_dir, "audio-levels-20ms-*") {
                if tree.is_dir(&folder) {
                    out.push((slug_dir.name().to_string(), label_dir.name().to_string(), folder));
                }
            }
        }
    }
    out
}

/// A rip of the same title with the same size that was probed. Py: same_rip's tuple.
struct SameRip {
    label: String,
    probe: PathKey,
    title: Value,
    bytes: Value,
}

/// (title, bytes) of an ok rip-summary.json or extract-summary.json (that order) naming a title and a size; None
/// otherwise, and for a file that cannot be read or parsed. Py: same_rip.summary.
fn rip_summary(tree: &dyn Tree, d: &PathKey) -> Option<(Value, Value)> {
    for fname in ["rip-summary.json", "extract-summary.json"] {
        let Ok(data) = read_json::<Value>(tree, &d.join(fname)) else { continue };
        let Some(object) = data.as_object() else { continue };
        let title = object.get("title").cloned().unwrap_or(Value::Null);
        let bytes = object.get("bytes").cloned().unwrap_or(Value::Null);
        if object.get("ok") == Some(&Value::Bool(true)) && !title.is_null() && py_truthy(&bytes) {
            return Some((title, bytes));
        }
    }
    None
}

/// Another rip folder of `slug_dir` (folder order, golden/ never seen) with an ffprobe-streams.json and the same
/// (title, bytes) as `label`'s summary. Py: same_rip.
fn same_rip(tree: &dyn Tree, slug_dir: &PathKey, label: &str) -> Option<SameRip> {
    let mine = rip_summary(tree, &slug_dir.join(label))?;
    for d in tree.list(slug_dir) {
        if !tree.is_dir(&d) || d.name() == label {
            continue;
        }
        let probe = d.join("ffprobe-streams.json");
        if tree.is_file(&probe) && rip_summary(tree, &d).as_ref() == Some(&mine) {
            return Some(SameRip { label: d.name().to_string(), probe, title: mine.0.clone(), bytes: mine.1.clone() });
        }
    }
    None
}

/// The audio-levels.ps1 job lines for a ripped title (EXP-13 addendum 9.1): the programme reference against every
/// other stream in a wanted language or with none (and the reference's own language), a dataset-only job per
/// unwanted language with two or more streams, the notes. The streams come from the rip's ffprobe-streams.json,
/// else from a rip of the same title and size, else from its mkvmerge.json; Err with the Python's message
/// (three lines when it says how to probe). A label named `golden` is refused. Py: levels_args.
pub fn levels_args(tree: &dyn Tree, slug_dir: &PathKey, label: &str) -> Result<Vec<String>, String> {
    let d = slug_dir.join(label);
    let shown = tree.display(&d);
    if label == GOLDEN {
        return Err(format!("{shown}: {GOLDEN_NOT_A_RIP}"));
    }
    let slug = tree.resolved_name(slug_dir);
    let probe = d.join("ffprobe-streams.json");
    let (audio, source) = if tree.is_file(&probe) {
        (ffprobe_audio(&read_json::<FfprobeStreams>(tree, &probe)?.streams), "ffprobe-streams.json".to_string())
    } else if let Some(sib) = same_rip(tree, slug_dir, label) {
        let streams = read_json::<FfprobeStreams>(tree, &sib.probe)?.streams;
        let source = format!(
            "{}/ffprobe-streams.json: a rip of the same title {} with the same size, {} bytes",
            sib.label,
            py_str(&sib.title),
            py_str(&sib.bytes)
        );
        (ffprobe_audio(&streams), source)
    } else if tree.is_file(&d.join("mkvmerge.json")) {
        let ident: MkvmergeIdent = read_json(tree, &d.join("mkvmerge.json"))?;
        let mut audio = Vec::new();
        for t in &ident.tracks {
            if t.r#type != "audio" {
                continue;
            }
            let p = &t.properties;
            let c = t.codec.as_str();
            let lossless = ["TrueHD", "DTS-HD Master Audio", "FLAC", "PCM"].iter().any(|x| c.contains(x));
            let channels_value = p.get("audio_channels");
            let channels = match channels_value {
                Some(v) if py_truthy(v) => py_int_value(v)?,
                _ => 0,
            };
            let language = match p.get("language") {
                Some(v) if py_truthy(v) => py_str(v),
                _ => "und".to_string(),
            };
            // " ".join(str(w) for w in [codec, f"{audio_channels} ch", track_name] if w)
            let mut what: Vec<String> = Vec::new();
            if !c.is_empty() {
                what.push(c.to_string());
            }
            what.push(format!("{} ch", channels_value.map_or_else(|| "None".to_string(), py_str)));
            if let Some(name) = p.get("track_name").filter(|v| py_truthy(v)) {
                what.push(py_str(name));
            }
            audio.push(AudioStream { stream: t.id, channels, lossless, language, description: what.join(" ") });
        }
        let source = "mkvmerge.json (track IDs: check them against ffprobe's stream numbers in the job's stream lines)";
        (audio, source.to_string())
    } else {
        return Err(format!(
            "{shown}: no ffprobe-streams.json or mkvmerge.json, and no other rip of the same title with the same size has one. Probe the rip first, then run args again:\nscripts/phase0/job.sh probe-title.ps1 probe-{label} -Slug {slug} -Label {label} -SkipRemux\nscripts/phase0/wait.sh probe-{label}"
        ));
    };
    if audio.is_empty() {
        return Err(format!("{shown}: the file has no audio stream"));
    }
    let (original, what) = original_language(tree, slug_dir);
    let (reference, why) = programme_reference(&audio, original);
    let mut first_group = vec![reference.clone()];
    let mut rest: Vec<AudioStream> = audio
        .iter()
        .filter(|a| a.stream != reference.stream && (wanted(a, original) || a.language == reference.language))
        .cloned()
        .collect();
    rest.sort_by_key(|a| a.stream);
    first_group.extend(rest);
    let mut groups = vec![first_group];
    let mut others: BTreeMap<String, Vec<AudioStream>> = BTreeMap::new();
    for a in &audio {
        if !wanted(a, original) {
            others.entry(a.language.clone()).or_default().push(a.clone());
        }
    }
    for streams in others.values() {
        if streams.len() >= 2 && !streams.iter().any(|a| a.stream == reference.stream) {
            let top = placed_first(streams);
            let mut group = vec![top.clone()];
            let mut rest: Vec<AudioStream> = streams.iter().filter(|a| a.stream != top.stream).cloned().collect();
            rest.sort_by_key(|a| a.stream);
            group.extend(rest);
            groups.push(group);
        }
    }
    let mut lines = vec![format!("{slug}/{label}: {} audio streams (from {source}); {what}", audio.len())];
    for a in &audio {
        lines.push(format!("  stream {}: {} lang={}", a.stream, a.description, a.language));
    }
    let mut codes = vec!["eng"];
    codes.extend(original.iter().copied());
    lines.push(format!(
        "  programme reference: stream {} ({}), placed first in {why} (spec 8.6 steps 3 and 4); candidates: every other stream in {} or with no language, whatever its language",
        reference.stream,
        reference.description,
        codes.join(", ")
    ));
    if !original.is_empty() && why != "the original language" {
        lines.push(format!(
            "  note: the file has no stream in the original language ({}): the reference may not be the programme (an English-only rip of a foreign film can hold only a commentary)",
            original.join("/")
        ));
    }
    if let Some(first) = rule_first_of_language(&audio, reference.stream) {
        lines.push(format!(
            "  note: the reference is not the first {} stream of the file (stream {}, {}, comes first): the position veto holds (row 0, D21), so no candidate of this title gets a commentary verdict (each is reference-unclear); measure as printed and say so in the report",
            reference.language, first.stream, first.description
        ));
    }
    let louder: Vec<String> = audio
        .iter()
        .filter(|a| NO_LANGUAGE.contains(&a.language.as_str()) && a.stream != reference.stream && (a.lossless || a.channels > 2))
        .map(|a| a.stream.to_string())
        .collect();
    if !reference.lossless && reference.channels <= 2 && !louder.is_empty() {
        lines.push(format!(
            "  note: the reference is a lossy stream of {} channel(s), and stream {} with no language code is lossless or has more than 2 channels: if it is the programme, the reference may be a commentary (EXP-13 addendum 9.1); measure as printed and say so in the report",
            reference.channels,
            louder.join(", ")
        ));
    }
    for g in &groups[1..] {
        let against: Vec<String> = g[1..].iter().map(|a| a.stream.to_string()).collect();
        lines.push(format!(
            "  dataset only: {} is not a wanted language; stream {} against {}",
            g[0].language,
            g[0].stream,
            against.join(",")
        ));
    }
    for g in &groups {
        let main = g[0].stream;
        let cands: Vec<String> = g[1..].iter().map(|a| a.stream.to_string()).collect();
        let log = if groups.len() > 1 {
            format!("audio-levels-{slug}-{label}-{main}")
        } else {
            format!("audio-levels-{slug}-{label}")
        };
        let mut job = format!("scripts/phase0/job.sh audio-levels.ps1 {log} -Slug {slug} -Label {label} -Main {main}");
        if !cands.is_empty() {
            job.push_str(&format!(" -Cand {}", cands.join(",")));
        }
        lines.push(job);
        lines.push(format!("scripts/phase0/wait.sh {log}"));
    }
    Ok(lines)
}

// ---------------------------------------------------------------------------------------------------------------
// The snapshot

/// The snapshot's "code" in v3.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotCode {
    pub rule_version: u32,
}

/// A v3 snapshot; field order is the file's key order. Py: write_snapshot's dict.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub format: String,
    pub created: String,
    /// Always "corpus/owner".
    pub corpus: String,
    pub code: SnapshotCode,
    /// The rule in force (defaults with the rule overrides).
    pub rule: Rule,
    /// The constants in force.
    pub constants: Constants,
    pub overrides: SnapshotOverrides,
    pub spec_row13: Requirements,
    /// Sorted by py_repr of each entry as a list (`sorted(..., key=str)`); each written as [slug, label, stream].
    pub fitted: Vec<FittedKey>,
    pub tracks: Vec<SnapshotTrack>,
    pub unlabelled_verdicts_note: String,
    /// track key -> verdict of every unlabelled candidate, in track order.
    pub unlabelled_verdicts: Ordered<Verdict>,
    pub unreadable: Vec<String>,
    /// The keys used in this run, sorted.
    pub measures_cache: BTreeMap<String, Measures>,
}

/// {"rule": {...}, "constants": {...}} of the --set values.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotOverrides {
    pub rule: Ordered<Num>,
    pub constants: Ordered<Num>,
}

/// One track row: the Python's `keep` keys in order, then owner_label. An unlabelled candidate's verdict,
/// swapped_verdict, before_veto and peer verdict are None (its verdict is in unlabelled_verdicts).
/// Py: write_snapshot's row.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotTrack {
    pub slug: String,
    pub label: String,
    pub folder: String,
    /// "reference" | "candidate".
    pub role: String,
    pub stream: i64,
    pub main: i64,
    pub md5: Option<String>,
    pub md5_rank: i64,
    /// Manifest strings.
    pub codec: Option<String>,
    pub channels: Option<String>,
    pub language: Option<String>,
    pub ref_fc: bool,
    /// None: no streams.json; "": the right reference; else what is wrong.
    pub ref_problem: Option<String>,
    pub excluded: bool,
    pub secondary: bool,
    pub verdict: Option<Verdict>,
    pub not_judged: String,
    pub duplicate_of: Option<String>,
    pub measures: Option<Measures>,
    pub key: Option<String>,
    pub kind: Option<String>,
    pub inherited_from: Option<String>,
    pub counts: bool,
    pub why_not_counted: String,
    pub outcome: String,
    pub cross_language: String,
    pub swapped_verdict: Option<Verdict>,
    pub swap_fail: bool,
    pub peer: Option<PeerRow>,
    /// (stream, description), written as a two-element list.
    pub ref_first: Option<(i64, String)>,
    /// The veto's reason when it holds, else "".
    pub veto: String,
    pub before_veto: Option<Verdict>,
    /// The owner's label entry as it is in the labels file.
    pub owner_label: Option<Value>,
}

/// A candidate's peer run in the snapshot: keys stream, verdict, measures, key. Py: peer_row.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PeerRow {
    pub stream: i64,
    pub verdict: Option<Verdict>,
    pub measures: Measures,
    pub key: Option<String>,
}

/// "<slug>/<label>/<folder>/<role>/<stream>". Py: track_key.
pub fn track_key(slug: &str, label: &str, folder: &str, role: &str, stream: i64) -> String {
    format!("{slug}/{label}/{folder}/{role}/{stream}")
}

/// A snapshot read back (v2 or v3): what the change blocks and the cache need. Py: load_snapshot's dict.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviousSnapshot {
    pub format: String,
    pub created: String,
    /// "rule version <n>" (v3) or "code <first 12 of audio_similarity_sha1>" (v2): what the Changes lines print.
    pub code_phrase: String,
    /// The rows as JSON (slug, label, folder, role, stream, verdict, kind, outcome are read).
    pub tracks: Vec<Value>,
    pub unlabelled_verdicts: serde_json::Map<String, Value>,
    /// v3 only; empty for v2.
    pub measures_cache: BTreeMap<String, Measures>,
    /// The whole document (for the unchanged-but-for-created test).
    pub raw: Value,
}

/// The snapshot in `text`, None when it is not JSON, not an object, or its format is neither v3 nor v2 (or a v3
/// file's measures_cache does not parse, or a part the change blocks need is missing, where the Python stopped with
/// a KeyError). The caller reads the file as plain "utf-8": a byte-order mark makes it unreadable, as in the Python.
/// Py: load_snapshot.
pub fn read_snapshot(text: &str) -> Option<PreviousSnapshot> {
    let raw: Value = serde_json::from_str(text).ok()?;
    let object = raw.as_object()?;
    let format = object.get("format")?.as_str()?.to_string();
    let code = object.get("code")?;
    let (code_phrase, measures_cache) = if format == SNAPSHOT_FORMAT {
        let cache: BTreeMap<String, Measures> = match object.get("measures_cache") {
            Some(value) => serde_json::from_value(value.clone()).ok()?,
            None => BTreeMap::new(),
        };
        (format!("rule version {}", py_str(code.get("rule_version")?)), cache)
    } else if format == SNAPSHOT_FORMAT_V2 {
        let sha = code.get("audio_similarity_sha1")?.as_str()?;
        (format!("code {}", sha.chars().take(12).collect::<String>()), BTreeMap::new())
    } else {
        return None;
    };
    let created = py_str(object.get("created")?);
    let tracks = object.get("tracks")?.as_array()?.clone();
    let unlabelled_verdicts = object.get("unlabelled_verdicts").and_then(Value::as_object).cloned().unwrap_or_default();
    Some(PreviousSnapshot { format, created, code_phrase, tracks, unlabelled_verdicts, measures_cache, raw })
}

// ---------------------------------------------------------------------------------------------------------------
// The harness

/// What the harness reads.
pub struct HarnessInput<'a> {
    /// The evidence root, printed as "corpus/owner".
    pub tree: &'a dyn Tree,
    /// exp13_requirements of the spec text (the caller exits 2 on its Err before building the tree).
    pub req: &'a Requirements,
    pub overrides: &'a Overrides,
    /// <corpus>/audio-dataset.json read back, when it is a snapshot.
    pub previous: Option<&'a PreviousSnapshot>,
    /// --compare: the path as given and the snapshot (the caller exits 2 when it is not one).
    pub compare: Option<(&'a str, &'a PreviousSnapshot)>,
    /// false only in the cache-off test.
    pub use_cache: bool,
    /// The snapshot's "created" (Ctx::now()).
    pub created: &'a str,
}

/// The run, up to the snapshot line.
#[derive(Clone, Debug, PartialEq)]
pub struct Harness {
    /// Sections 1 to 6 of the report (spec 3.4's rewritten lines in their new form), then the change blocks;
    /// without the blank line, the "snapshot:" line and the RESULT line.
    pub lines: Vec<String>,
    /// The reasons for exit 1.
    pub failures: Vec<String>,
    /// What could not be checked.
    pub unchecked: Vec<String>,
    pub snapshot: Snapshot,
    pub computed: usize,
    pub reused: usize,
}

/// A labels or fitted file does not validate: the lines to print (each problem, then the RESULT line); exit 2.
#[derive(Clone, Debug, PartialEq)]
pub enum HarnessError {
    Labels(Vec<String>),
}

/// The tuning harness over the evidence: the labels and the fitted list, the measures with the default rule (the
/// cache filled), the trial with the overrides when given, the report, the change blocks, what was not checked,
/// the snapshot. Py: audio_dataset.main's body from loading the labels to the change blocks.
pub fn harness(input: &HarnessInput) -> Result<Harness, HarnessError> {
    let tree = input.tree;
    let root = PathKey::root();
    let mut labelled: BTreeMap<String, Labels> = BTreeMap::new();
    let mut bad: Vec<String> = Vec::new();
    for f in tree.glob(&root, "*/audio-labels.json") {
        match load(tree, &f) {
            Ok(data) => {
                labelled.insert(f.parent().name().to_string(), data);
            }
            Err(e) => bad.push(e),
        }
    }
    let fitted = match load_fitted(tree, &root, Some(input.req.fitting_slug.as_str())) {
        Ok(fitted) => fitted,
        Err(e) => {
            bad.push(e);
            Fitted::new()
        }
    };
    if !bad.is_empty() {
        bad.push(LABELS_INVALID.to_string());
        return Err(HarnessError::Labels(bad));
    }
    let mut titles: BTreeMap<(String, String), TitleFacts> = BTreeMap::new();
    for (slug, data) in &labelled {
        for (label, facts) in data.titles.iter() {
            titles.insert((slug.clone(), label.to_string()), facts.clone());
        }
    }
    let proxies: BTreeSet<(String, String)> = tree
        .glob(&root, "*/*/audio-levels-20ms-*/audio-proxy.json")
        .iter()
        .map(|p| (p.components()[0].clone(), p.components()[1].clone()))
        .collect();
    // A v2 baseline's measures are never a seed: read_snapshot leaves its cache empty (spec 3.3).
    let mut cache: BTreeMap<String, Measures> = input.previous.map(|p| p.measures_cache.clone()).unwrap_or_default();
    let found = find_folders(tree);
    let (default_rule, default_consts) = (Rule::default(), Constants::default());
    let mut pass = measure(tree, &found, &default_rule, &default_consts, &mut cache, input.use_cache);
    join_labels(&mut pass.tracks, &labelled, &fitted);
    let shown = input.overrides.shown();
    let base_verdicts = if shown.is_empty() { None } else { Some(run_verdicts(&pass.tracks)) };
    let (rule, consts) = input.overrides.effective();
    if !shown.is_empty() {
        // The first pass has put the baseline's measures into the cache already.
        let first_computed = pass.computed;
        pass = measure(tree, &found, &rule, &consts, &mut cache, input.use_cache);
        join_labels(&mut pass.tracks, &labelled, &fitted);
        pass.computed += first_computed;
    }
    let rips = ripped(tree);
    let report = report(tree, &pass, &labelled, input.req, &titles, &shown, &proxies, &rips);
    let mut lines = report.lines;
    let now = run_verdicts(&pass.tracks);
    let mut gone: Vec<String> = Vec::new();
    if let Some(base) = &base_verdicts {
        let names: Vec<String> = shown.iter().map(|(key, value)| format!("{}={value}", key.to_uppercase())).collect();
        lines.push(String::new());
        lines.extend(changes(base, &now, &format!("Changes against the baseline rule ({}):", names.join(", "))).0);
    }
    if let Some((path, other)) = input.compare {
        let title = format!("Changes against {path} ({}, {}):", other.created, other.code_phrase);
        lines.push(String::new());
        lines.extend(changes(&snapshot_verdicts(other), &now, &title).0);
    }
    if let Some(previous) = input.previous {
        if shown.is_empty() {
            let title =
                format!("Changes since the previous snapshot ({}, {}):", previous.created, previous.code_phrase);
            let (text, g) = changes(&snapshot_verdicts(previous), &now, &title);
            lines.push(String::new());
            lines.extend(text);
            gone = g;
        }
    }
    let mut unchecked = Vec::new();
    if found.is_empty() {
        unchecked.push(format!("no level folder under {CORPUS}"));
    }
    if !pass.unreadable.is_empty() {
        unchecked.push(format!("{} folder(s) cannot be read", pass.unreadable.len()));
    }
    if !report.orphans.is_empty() {
        unchecked.push(format!("{} labelled track(s) not measured", report.orphans.len()));
    }
    if !gone.is_empty() {
        unchecked.push(format!("{} track(s) of the previous snapshot are gone", gone.len()));
    }
    let snapshot = build_snapshot(input, &pass, rule, consts, &fitted);
    Ok(Harness {
        lines,
        failures: report.failures,
        unchecked,
        snapshot,
        computed: pass.computed,
        reused: pass.reused,
    })
}

/// Where the snapshot goes. Py: main's choice of `out`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Given,
    Trial,
    Unchecked,
    Baseline,
}

/// --snapshot given; else overrides: Trial; else something unchecked other than "not measured": Unchecked; else
/// Baseline. Py: main's `out`.
pub fn destination(given: bool, overrides: bool, unchecked: &[String]) -> Destination {
    if given {
        Destination::Given
    } else if overrides {
        Destination::Trial
    } else if unchecked.iter().any(|u| !u.ends_with("not measured")) {
        Destination::Unchecked
    } else {
        Destination::Baseline
    }
}

/// The phrase after the path. Py: main's `why`.
pub fn why_phrase(dest: Destination, baseline_shown: &str, unchecked: &[String]) -> String {
    match dest {
        Destination::Given | Destination::Baseline => String::new(),
        Destination::Trial => " (a trial: the baseline is not replaced)".to_string(),
        Destination::Unchecked => {
            let keep: Vec<&str> =
                unchecked.iter().filter(|u| !u.ends_with("not measured")).map(String::as_str).collect();
            format!(
                " (the baseline {baseline_shown} was NOT replaced: {}; to replace it on purpose, run again with --snapshot {baseline_shown})",
                keep.join("; ")
            )
        }
    }
}

/// "snapshot: <out><why>" plus " (unchanged but for its time: not rewritten)" when not written.
pub fn snapshot_line(out_shown: &str, why: &str, written: bool) -> String {
    let unchanged = if written { "" } else { " (unchanged but for its time: not rewritten)" };
    format!("snapshot: {out_shown}{why}{unchanged}")
}

/// The RESULT line and the exit code: 1 on failures (what was not checked appended), 3 when something was not
/// checked and --allow-unmeasured was not given, 0 otherwise. Py: main's last lines.
pub fn result_line(failures: &[String], unchecked: &[String], allow_unmeasured: bool) -> (String, i32) {
    if !failures.is_empty() {
        let mut parts: Vec<String> = failures.to_vec();
        if !unchecked.is_empty() {
            parts.push(format!("not checked: {}", unchecked.join(", ")));
        }
        (format!("RESULT FAILED: {}", parts.join("; ")), 1)
    } else if !unchecked.is_empty() && !allow_unmeasured {
        (format!("RESULT FAILED: not everything was checked: {}{HINT}", unchecked.join(", ")), 3)
    } else if unchecked.is_empty() {
        ("RESULT ok".to_string(), 0)
    } else {
        (format!("RESULT ok (not checked, allowed by --allow-unmeasured: {})", unchecked.join(", ")), 0)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The tracks (private: the Python's track dicts)

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Reference,
    Candidate,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Reference => "reference",
            Role::Candidate => "candidate",
        }
    }
}

/// One stream in one level folder, every key the Python's track dict gets in measure() and join_labels().
#[derive(Clone, Debug)]
struct Track {
    slug: String,
    label: String,
    folder: String,
    main: i64,
    ref_fc: bool,
    ref_problem: Option<String>,
    excluded: bool,
    secondary: bool,
    n_cands: usize,
    ref_first: Option<(i64, String)>,
    veto: String,
    veto_unchecked: String,
    role: Role,
    stream: i64,
    md5: Option<String>,
    md5_rank: i64,
    codec: Option<String>,
    channels: Option<String>,
    language: Option<String>,
    /// (k, n, title): the k-th of the file's n audio streams.
    position: Option<(usize, usize, String)>,
    verdict: Option<Verdict>,
    measures: Option<Measures>,
    key: Option<String>,
    not_judged: String,
    duplicate_of: Option<String>,
    cross_language: String,
    swapped_verdict: Option<Verdict>,
    peer: Option<PeerRow>,
    before_veto: Option<Verdict>,
    unjudged: String,
    also_in: Vec<String>,
    inherited_from: Option<String>,
    ambiguous: Option<String>,
    label_entry: Option<LabelEntry>,
    kind: Option<String>,
    group: Option<&'static str>,
    why_not_counted: String,
    counts: bool,
    outcome: String,
    swap_fail: bool,
}

impl Track {
    /// The md5 when it is truthy (the Python's `t["md5"]` tests).
    fn md5(&self) -> Option<&str> {
        self.md5.as_deref().filter(|m| !m.is_empty())
    }

    fn key(&self) -> String {
        track_key(&self.slug, &self.label, &self.folder, self.role.as_str(), self.stream)
    }

    /// `t["kind"]` truthy.
    fn labelled(&self) -> bool {
        self.kind.as_deref().is_some_and(|k| !k.is_empty())
    }
}

/// One pass of measure(): the tracks, the folders that cannot be read, the Measurer's counts and keys.
struct Pass {
    tracks: Vec<Track>,
    unreadable: Vec<String>,
    used: BTreeMap<String, Measures>,
    computed: usize,
    reused: usize,
}

/// Every reference and candidate of every folder under one rule and one set of constants. A track is one stream in
/// one folder; the same stream in several folders of a title gives several tracks, the one in the folder with the
/// most candidates primary (the first of equals), the others secondary. Py: measure.
fn measure(
    tree: &dyn Tree,
    found: &[(String, String, PathKey)],
    rule: &Rule,
    consts: &Constants,
    cache: &mut BTreeMap<String, Measures>,
    use_cache: bool,
) -> Pass {
    let mut meas = Measurer::new(cache, consts, use_cache);
    let mut tracks: Vec<Track> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    let mut originals: BTreeMap<String, &'static [&'static str]> = BTreeMap::new();
    for (slug, label, folder) in found {
        if !originals.contains_key(slug) {
            originals.insert(slug.clone(), original_language(tree, &folder.parent().parent()).0);
        }
        let original = originals[slug];
        let name = folder.name().to_string();
        if !tree.is_file(&folder.join("manifest.txt")) && tree.is_file(&folder.join("streams.json")) {
            unreadable.push(format!("{slug}/{label}/{name}: {LEVELS_NOT_HERE}"));
            continue;
        }
        let res = match results(tree, folder, rule, consts, &mut meas) {
            Ok(res) => res,
            Err(e) => {
                unreadable.push(format!("{slug}/{label}/{name}: {e}"));
                continue;
            }
        };
        let streams = match read_streams(tree, folder) {
            Ok(streams) => streams,
            Err(e) => {
                unreadable.push(format!("{slug}/{label}/{name}: {e}"));
                continue;
            }
        };
        // int(mi["main"]) and int(c["stream"]): results() has used both as numbers already (the veto, the level
        // files' names); a value that is not one makes the folder unreadable with int()'s text, where the Python
        // stopped with a traceback.
        let mut numbers: Vec<i64> = Vec::new();
        let mut not_int: Option<String> = None;
        let texts = std::iter::once(res.main.get("main").map(String::as_str).unwrap_or(""))
            .chain(res.candidates.iter().map(|c| c.stream.as_str()));
        for text in texts {
            match py_int(text) {
                Some(n) => numbers.push(n),
                None => {
                    not_int = Some(format!("invalid literal for int() with base 10: {}", py_repr_str(text)));
                    break;
                }
            }
        }
        if let Some(e) = not_int {
            unreadable.push(format!("{slug}/{label}/{name}: {e}"));
            continue;
        }
        let main = numbers[0];
        let mi = &res.main;
        let pos = audio_positions(streams.as_deref());
        let problem = reference_problem(streams.as_deref(), main, original);
        let mut order: Vec<(i64, Option<&str>)> = vec![(main, mi.get("md5").map(String::as_str))];
        for (c, n) in res.candidates.iter().zip(&numbers[1..]) {
            order.push((*n, c.info.get("md5").map(String::as_str)));
        }
        // {n: sum(1 for m, h in order[:k] if h and h == md5)}: the place of the stream among the copies of its packets
        let mut ranks: BTreeMap<i64, i64> = BTreeMap::new();
        for (k, (n, md5)) in order.iter().enumerate() {
            let same = order[..k].iter().filter(|(_, h)| h.is_some_and(|h| !h.is_empty() && Some(h) == *md5)).count();
            ranks.insert(*n, same as i64);
        }
        let base = Track {
            slug: slug.clone(),
            label: label.clone(),
            folder: name.clone(),
            main,
            ref_fc: mi.get("fc").map(String::as_str) == Some("yes"),
            ref_problem: problem.clone(),
            excluded: problem.as_deref().is_some_and(|p| !p.is_empty()),
            secondary: false,
            n_cands: res.candidates.len(),
            ref_first: first_of_language(streams.as_deref(), main),
            veto: if res.veto.holds == Some(true) { res.veto.why.clone() } else { String::new() },
            veto_unchecked: if res.veto.holds.is_none() { res.veto.why.clone() } else { String::new() },
            role: Role::Reference,
            stream: main,
            md5: mi.get("md5").cloned(),
            md5_rank: 0,
            codec: mi.get("codec").cloned(),
            channels: mi.get("channels").cloned(),
            language: mi.get("language").cloned(),
            position: pos.get(&main).cloned(),
            verdict: None,
            measures: None,
            key: None,
            not_judged: String::new(),
            duplicate_of: None,
            cross_language: String::new(),
            swapped_verdict: None,
            peer: None,
            before_veto: None,
            unjudged: String::new(),
            also_in: Vec::new(),
            inherited_from: None,
            ambiguous: None,
            label_entry: None,
            kind: None,
            group: None,
            why_not_counted: String::new(),
            counts: false,
            outcome: String::new(),
            swap_fail: false,
        };
        tracks.push(base.clone());
        for (c, &n) in res.candidates.iter().zip(&numbers[1..]) {
            let back = c.measures.as_ref().and_then(swapped);
            tracks.push(Track {
                role: Role::Candidate,
                stream: n,
                md5: c.info.get("md5").cloned(),
                md5_rank: ranks[&n],
                codec: c.info.get("codec").cloned(),
                channels: c.info.get("channels").cloned(),
                language: c.info.get("language").cloned(),
                position: pos.get(&n).cloned(),
                verdict: Some(c.verdict),
                measures: c.measures.clone(),
                key: if c.measures.is_some() { c.key.clone() } else { None },
                not_judged: c.not_judged.clone(),
                duplicate_of: c.duplicate_of.clone(),
                cross_language: c.cross_language.clone(),
                swapped_verdict: back.as_ref().map(|b| verdict(b, rule)),
                peer: c.peer.as_ref().map(peer_row),
                before_veto: c.before_veto,
                unjudged: not_judged_by_app(c.info.get("language").map(String::as_str), original),
                ..base.clone()
            });
        }
    }
    let mut groups: BTreeMap<(String, String, Role, i64), Vec<usize>> = BTreeMap::new();
    for (i, t) in tracks.iter().enumerate() {
        if !t.excluded {
            groups.entry((t.slug.clone(), t.label.clone(), t.role, t.stream)).or_default().push(i);
        }
    }
    for same in groups.values() {
        let mut primary = same[0]; // max() keeps the first of equals: folder name order
        for &i in &same[1..] {
            if tracks[i].n_cands > tracks[primary].n_cands {
                primary = i;
            }
        }
        for &i in same {
            let also_in: Vec<String> = same.iter().filter(|&&o| o != i).map(|&o| tracks[o].folder.clone()).collect();
            tracks[i].secondary = i != primary;
            tracks[i].also_in = also_in;
        }
    }
    let Measurer { used, computed, reused, .. } = meas;
    Pass { tracks, unreadable, used, computed, reused }
}

/// {stream: (k, n, title)}: the stream is the k-th of the file's n audio streams (a player's audio track k).
/// Py: audio_positions.
fn audio_positions(streams: Option<&[FfprobeStream]>) -> BTreeMap<i64, (usize, usize, String)> {
    let audio: Vec<&FfprobeStream> =
        streams.unwrap_or(&[]).iter().filter(|s| s.codec_type.as_deref() == Some("audio")).collect();
    let n = audio.len();
    audio
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let title = s.tags.as_ref().and_then(|tags| tags.get("title")).cloned().unwrap_or_default();
            (s.index, (k + 1, n, title))
        })
        .collect()
}

/// None when there is no streams.json to check against; "" when `main` is the title's programme reference, or the
/// stream placed first in a language that is not wanted (a folder measured for the dataset only); otherwise what is
/// wrong. Py: reference_problem.
fn reference_problem(streams: Option<&[FfprobeStream]>, main: i64, original: &[&str]) -> Option<String> {
    let streams = streams?;
    let audio = ffprobe_audio(streams);
    let Some(mine) = audio.iter().find(|a| a.stream == main) else {
        return Some(format!("stream {main} is not an audio stream of the file (streams.json)"));
    };
    let (reference, why) = programme_reference(&audio, original);
    if reference.stream == main {
        return Some(String::new());
    }
    if !wanted(mine, original) {
        let same_language: Vec<AudioStream> = audio.iter().filter(|a| a.language == mine.language).cloned().collect();
        if placed_first(&same_language).stream == main {
            return Some(String::new());
        }
    }
    Some(format!(
        "measured against stream {main} ({}), but the top-ranked {} stream is {} ({}), the title's programme reference (placed first in {why}; EXP-13 addendum, section 9.1)",
        mine.description, reference.language, reference.stream, reference.description
    ))
}

/// "" when the app judges a candidate in this language (English or the work's original language); otherwise why it
/// never does. Py: not_judged_by_app.
fn not_judged_by_app(language: Option<&str>, original: &[&str]) -> String {
    let lang = language.unwrap_or("");
    if lang == "eng" || original.contains(&lang) {
        return String::new();
    }
    if NO_LANGUAGE.contains(&lang) {
        let shown = if lang.is_empty() { "none" } else { lang };
        return format!("no language code ({shown}): the app never judges it, measured for the dataset only");
    }
    format!("{lang} is not a wanted language: the app never judges it, measured for the dataset only")
}

/// A candidate's peer run as stored in the snapshot. Py: peer_row.
fn peer_row(peer: &Peer) -> PeerRow {
    PeerRow {
        stream: py_int(&peer.stream).expect("results writes the peer's stream number"),
        verdict: Some(peer.verdict),
        measures: peer.measures.clone(),
        key: peer.key.clone(),
    }
}

fn in_drop(v: Option<Verdict>) -> bool {
    v.is_some_and(|v| DROP.contains(&v))
}

fn in_pattern(v: Option<Verdict>) -> bool {
    v.is_some_and(|v| PATTERN.contains(&v))
}

/// Gives every track its label (its own, or inherited from a track of another rip of the same disc with the same
/// packets and the same place among the copies of those packets), whether it counts for experiment 13, its
/// outcome, and whether it is a swapped-reference failure. Py: join_labels.
fn join_labels(tracks: &mut [Track], labelled: &BTreeMap<String, Labels>, fitted: &Fitted) {
    let mut direct: BTreeMap<(String, String, i64), LabelEntry> = BTreeMap::new();
    for (slug, data) in labelled {
        for t in &data.tracks {
            direct.insert((slug.clone(), t.label.clone(), t.stream), t.clone());
        }
    }
    let mut sources: BTreeMap<(String, String, i64), BTreeMap<(String, i64), LabelEntry>> = BTreeMap::new();
    for t in tracks.iter() {
        let Some(lab) = direct.get(&(t.slug.clone(), t.label.clone(), t.stream)) else { continue };
        if t.role == Role::Candidate && !t.excluded {
            if let Some(md5) = t.md5() {
                sources
                    .entry((t.slug.clone(), md5.to_string(), t.md5_rank))
                    .or_default()
                    .insert((t.label.clone(), t.stream), lab.clone());
            }
        }
    }
    // The packets of a track a threshold was fitted on never count, whichever rip of the title they are in.
    let fitted_packets: BTreeSet<(String, String)> = tracks
        .iter()
        .filter(|t| is_fitted(fitted, &t.slug, &t.label, t.stream))
        .filter_map(|t| t.md5().map(|m| (t.slug.clone(), m.to_string())))
        .collect();
    let mut counted: BTreeSet<(String, String)> = BTreeSet::new();
    let mut order: Vec<usize> = (0..tracks.len()).collect();
    order.sort_by(|&a, &b| {
        let (x, y) = (&tracks[a], &tracks[b]);
        (&x.slug, &x.label, x.secondary, &x.folder, x.stream).cmp(&(&y.slug, &y.label, y.secondary, &y.folder, y.stream))
    });
    for i in order {
        let t = &tracks[i];
        let mut lab: Option<LabelEntry> = direct.get(&(t.slug.clone(), t.label.clone(), t.stream)).cloned();
        let mut inherited_from: Option<String> = None;
        let mut ambiguous: Option<String> = None;
        if lab.is_none() && t.role == Role::Candidate && !t.excluded {
            if let Some(md5) = t.md5() {
                let src: Vec<(&(String, i64), &LabelEntry)> = sources
                    .get(&(t.slug.clone(), md5.to_string(), t.md5_rank))
                    .map(|m| m.iter().filter(|((label, _), _)| *label != t.label).collect())
                    .unwrap_or_default();
                let kinds: BTreeSet<&str> = src.iter().map(|(_, v)| v.kind.as_str()).collect();
                if kinds.len() == 1 {
                    let ((src_label, src_stream), first) = src[0]; // sorted by (label, stream)
                    inherited_from = Some(format!("{src_label} stream {src_stream}"));
                    lab = Some(first.clone());
                } else if !kinds.is_empty() {
                    let names: Vec<String> =
                        src.iter().map(|((label, stream), v)| format!("{label} stream {stream} ({})", v.kind)).collect();
                    ambiguous = Some(format!("same packets as {}", names.join(", ")));
                }
            }
        }
        let kind = lab.as_ref().map(|e| e.kind.clone());
        let group: Option<&'static str> = kind.as_deref().and_then(kind_of).map(|(group, _, _)| group);
        let mut why = if t.role != Role::Candidate || t.excluded || t.secondary {
            "not a judged track of its title".to_string()
        } else if !t.unjudged.is_empty() {
            t.unjudged.clone()
        } else if !t.veto_unchecked.is_empty() {
            format!("the position veto (row 0, D21) is {}", t.veto_unchecked)
        } else if let Some(entry) = &lab {
            match &inherited_from {
                Some(from) => format!("its label is that of {from} (same packets), which counts there"),
                None => not_counted_because(entry, &t.slug, fitted),
            }
        } else {
            "no label".to_string()
        };
        if why.is_empty() {
            if let Some(md5) = t.md5() {
                let packets = (t.slug.clone(), md5.to_string());
                if fitted_packets.contains(&packets) {
                    why = "the same packets as a track the thresholds were fitted on".to_string();
                } else if counted.contains(&packets) {
                    why = "the same packets already count on this disc".to_string();
                }
            }
        }
        let counts = why.is_empty();
        if counts {
            if let Some(md5) = t.md5() {
                counted.insert((t.slug.clone(), md5.to_string()));
            }
        }
        let v = t.verdict;
        let outcome = if t.excluded {
            if t.role == Role::Reference { "wrong reference" } else { "excluded" }.to_string()
        } else if t.role == Role::Reference {
            match (kind.as_deref(), group) {
                (None, _) | (Some("main"), _) => "reference".to_string(),
                (_, Some("unwanted")) | (_, Some("duplicate")) => "reference is unwanted audio".to_string(),
                (Some(k), _) => format!("reference labelled {k}"),
            }
        } else if lab.is_none() {
            "unlabelled".to_string()
        } else if !t.unjudged.is_empty() {
            "not judged by the app".to_string()
        } else if group == Some("programme") && (in_drop(v) || in_pattern(v)) {
            "FAIL".to_string()
        } else if matches!(group, Some("unwanted") | Some("duplicate")) && !in_drop(v) {
            "kept".to_string()
        } else if group == Some("unwanted")
            && v == Some(Verdict::Commentary)
            && !kind.as_deref().is_some_and(|k| COMMENTARY_KINDS.contains(&k))
        {
            "dropped as a commentary".to_string()
        } else if matches!(group, Some("ask") | Some("unknown")) && (in_drop(v) || in_pattern(v)) {
            let who = if group == Some("ask") { "undecided" } else { "unknown" };
            let what = if in_drop(v) { "drop verdict" } else { "commentary pattern" };
            format!("{who} kind, {what}")
        } else {
            "agrees".to_string()
        };
        let t = &mut tracks[i];
        t.label_entry = lab;
        t.inherited_from = inherited_from;
        t.ambiguous = ambiguous;
        t.kind = kind;
        t.group = group;
        t.why_not_counted = why;
        t.counts = counts;
        t.outcome = outcome;
    }
    // A swapped reference: the same pair read the other way round. Only for labelled candidates; the reference is
    // programme audio unless the owner labelled it otherwise.
    let ref_kind: BTreeMap<(String, String, String), Option<String>> = tracks
        .iter()
        .filter(|t| t.role == Role::Reference)
        .map(|t| ((t.slug.clone(), t.label.clone(), t.folder.clone()), t.kind.clone()))
        .collect();
    for t in tracks.iter_mut() {
        let k = ref_kind.get(&(t.slug.clone(), t.label.clone(), t.folder.clone())).cloned().flatten();
        let programme_ref = match k.as_deref() {
            None => true,
            Some(k) => kind_of(k).map(|(group, _, _)| group) == Some("programme"),
        };
        t.swap_fail = t.role == Role::Candidate
            && !t.excluded
            && t.unjudged.is_empty()
            && t.kind.is_some()
            && t.swapped_verdict == Some(Verdict::Commentary)
            && programme_ref;
    }
}

/// The tracks the table and the counts use: one per stream of a title, from a folder with the right reference.
/// Py: judged.
fn judged(tracks: &[Track]) -> Vec<&Track> {
    tracks.iter().filter(|t| !t.excluded && !t.secondary).collect()
}

/// Experiment 13's numbers. Py: coverage's dict c.
struct Coverage {
    commentaries: usize,
    commentary_discs: usize,
    commentary_features: usize,
    commentary_audible: usize,
    partial: usize,
    ad: usize,
    programme: usize,
    derived_cores: usize,
    programme_separate: usize,
    no_centre_titles: usize,
    dialogue_heavy_titles: usize,
    /// isolated-score, music-effects, same-language-dub, alternate-narration.
    others: [usize; 4],
    verdict_discs: usize,
    dubs: usize,
    dub_discs: usize,
    dub_heavy: usize,
    dub_pattern: usize,
    cross_comm: usize,
    cross_comm_pattern: usize,
    cross_prog: usize,
    swapped: usize,
    swap_fails: usize,
}

fn kind_is(t: &Track, kind: &str) -> bool {
    t.kind.as_deref() == Some(kind)
}

fn feature(titles: &BTreeMap<(String, String), TitleFacts>, t: &Track) -> bool {
    titles.get(&(t.slug.clone(), t.label.clone())).is_some_and(|f| f.title_kind == "feature")
}

fn heavy(titles: &BTreeMap<(String, String), TitleFacts>, t: &Track) -> bool {
    titles.get(&(t.slug.clone(), t.label.clone())).is_some_and(|f| f.dialogue_heavy == Some(true))
}

/// Experiment 13's counts over the tracks that count, the answer as it stands, and the false verdicts (programme
/// tracks with the commentary pattern). Py: coverage.
fn coverage(
    tracks: &[Track],
    req: &Requirements,
    titles: &BTreeMap<(String, String), TitleFacts>,
) -> (Coverage, String, BTreeSet<(String, String, i64)>) {
    let good = judged(tracks);
    let cnt: Vec<&Track> = good.iter().copied().filter(|t| t.counts).collect();
    let comm: Vec<&Track> =
        cnt.iter().copied().filter(|t| t.kind.as_deref().is_some_and(|k| COMMENTARY_KINDS.contains(&k))).collect();
    let prog: Vec<&Track> = cnt.iter().copied().filter(|t| t.group == Some("programme") && !kind_is(t, "main")).collect();
    let dub: Vec<&Track> = cnt.iter().copied().filter(|t| kind_is(t, "dub")).collect();
    let c = Coverage {
        commentaries: comm.len(),
        commentary_discs: comm.iter().map(|t| t.slug.as_str()).collect::<BTreeSet<_>>().len(),
        commentary_features: comm.iter().filter(|t| feature(titles, t)).count(),
        commentary_audible: comm
            .iter()
            .filter(|t| t.label_entry.as_ref().and_then(|e| e.film_audible) == Some(true))
            .count(),
        partial: comm.iter().filter(|t| kind_is(t, "partial-commentary")).count(),
        ad: cnt.iter().filter(|t| kind_is(t, "audio-description")).count(),
        programme: prog.len(),
        derived_cores: prog.iter().filter(|t| kind_is(t, "derived-core")).count(),
        programme_separate: prog.iter().filter(|t| !kind_is(t, "derived-core") && !kind_is(t, "dub")).count(),
        no_centre_titles: cnt
            .iter()
            .filter(|t| !t.ref_fc)
            .map(|t| (t.slug.as_str(), t.label.as_str(), t.main))
            .collect::<BTreeSet<_>>()
            .len(),
        dialogue_heavy_titles: cnt
            .iter()
            .filter(|t| heavy(titles, t))
            .map(|t| (t.slug.as_str(), t.label.as_str()))
            .collect::<BTreeSet<_>>()
            .len(),
        others: OTHER_KINDS.map(|k| cnt.iter().filter(|t| kind_is(t, k)).count()),
        verdict_discs: comm
            .iter()
            .filter(|t| t.verdict == Some(Verdict::Commentary))
            .map(|t| t.slug.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        dubs: dub.len(),
        dub_discs: dub.iter().map(|t| t.slug.as_str()).collect::<BTreeSet<_>>().len(),
        dub_heavy: dub
            .iter()
            .filter(|t| heavy(titles, t))
            .map(|t| (t.slug.as_str(), t.label.as_str()))
            .collect::<BTreeSet<_>>()
            .len(),
        dub_pattern: dub.iter().filter(|t| in_pattern(t.verdict)).count(),
        cross_comm: comm.iter().filter(|t| !t.cross_language.is_empty()).count(),
        cross_comm_pattern: comm.iter().filter(|t| t.verdict == Some(Verdict::CommentaryCrossLanguage)).count(),
        cross_prog: prog.iter().filter(|t| !t.cross_language.is_empty()).count(),
        swapped: good
            .iter()
            .filter(|t| t.role == Role::Candidate && t.labelled() && t.swapped_verdict.is_some() && t.unjudged.is_empty())
            .count(),
        swap_fails: tracks.iter().filter(|t| t.swap_fail).count(),
    };
    let false_tracks: BTreeSet<(String, String, i64)> = tracks
        .iter()
        .filter(|t| t.outcome == "FAIL" && in_pattern(t.verdict))
        .map(|t| (t.slug.clone(), t.label.clone(), t.stream))
        .collect();
    let needs: [(&str, usize, i64); 8] = [
        ("commentaries", c.commentaries, req.commentaries),
        ("discs with a commentary", c.commentary_discs, req.commentary_discs),
        ("commentaries on feature films", c.commentary_features, req.commentary_features),
        ("commentaries with the film clearly audible", c.commentary_audible, req.commentary_audible),
        ("programme tracks", c.programme, req.programme),
        ("separately encoded programme tracks", c.programme_separate, req.programme_separate),
        ("titles whose reference has no centre channel", c.no_centre_titles, req.no_centre_titles),
        ("dialogue-heavy titles", c.dialogue_heavy_titles, req.dialogue_heavy_titles),
    ];
    let missing: Vec<&str> = needs.iter().filter(|(_, have, need)| (*have as i64) < *need).map(|(name, _, _)| *name).collect();
    let answer = if !false_tracks.is_empty() {
        format!("no: {} programme track(s) with the commentary pattern", false_tracks.len())
    } else if !missing.is_empty() {
        format!("not answered: too few {}", missing.join(", "))
    } else if (c.verdict_discs as i64) < req.verdict_discs {
        format!("no: commentary verdicts on {} discs, {} needed", c.verdict_discs, req.verdict_discs)
    } else {
        YES.to_string()
    };
    (c, answer, false_tracks)
}

/// The nine verdict counts of `rows`, each right-aligned in 5, joined by spaces.
fn verdict_counts(rows: &[&Track]) -> String {
    Verdict::ALL
        .iter()
        .map(|v| format!("{:>5}", rows.iter().filter(|t| t.verdict == Some(*v)).count()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Kind against verdict over the judged, labelled candidates. Py: table.
fn table(tracks: &[Track]) -> Vec<String> {
    let cands: Vec<&Track> = judged(tracks).into_iter().filter(|t| t.role == Role::Candidate).collect();
    let header: Vec<String> = SHORT.iter().map(|s| format!("{s:>5}")).collect();
    let mut lines = vec![format!("  {:<20} {:<7} {}  total", "kind", "wanted", header.join(" "))];
    for &(kind, _, wanted, _) in KINDS.iter() {
        let rows: Vec<&Track> = cands.iter().copied().filter(|t| kind_is(t, kind) && t.unjudged.is_empty()).collect();
        if !rows.is_empty() {
            lines.push(format!("  {kind:<20} {wanted:<7} {}  {:>5}", verdict_counts(&rows), rows.len()));
        }
    }
    let rest = cands.iter().filter(|t| t.kind.is_none()).count();
    let dashes: Vec<String> = Verdict::ALL.iter().map(|_| format!("{:>5}", "-")).collect();
    lines.push(format!("  {:<20} {:<7} {}  {:>5}   verdicts hidden until labelled", "(unlabelled)", "", dashes.join(" "), rest));
    let skipped: Vec<&Track> = cands.iter().copied().filter(|t| t.labelled() && !t.unjudged.is_empty()).collect();
    if !skipped.is_empty() {
        lines.push(format!(
            "  {:<20} {:<7} {}  {:>5}   no language code or not a wanted language: the app never judges them (notes below)",
            "(not judged)",
            "",
            verdict_counts(&skipped),
            skipped.len()
        ));
    }
    let columns: Vec<String> = SHORT.iter().zip(Verdict::ALL.iter()).map(|(s, v)| format!("{s} = {}", v.as_str())).collect();
    lines.push(format!("  columns: {}", columns.join(", ")));
    lines
}

/// "None" for an absent verdict, as the Python's f-strings print it.
fn vstr(v: Option<Verdict>) -> &'static str {
    v.map_or("None", Verdict::as_str)
}

/// "None" for an absent string.
fn ostr(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("None")
}

/// Py: name.
fn name(t: &Track) -> String {
    format!("{}/{} stream {}", t.slug, t.label, t.stream)
}

/// The track's name, with its folder when the stream is measured in more than one. Py: where.
fn where_of(t: &Track) -> String {
    if t.also_in.is_empty() {
        name(t)
    } else {
        format!("{} (in {})", name(t), t.folder)
    }
}

/// Py: describe.
fn describe(t: &Track) -> String {
    let at = match &t.position {
        Some((k, n, title)) => {
            let named = if title.is_empty() { String::new() } else { format!(", named \"{title}\"") };
            format!("audio track {k} of {n} in a player{named}")
        }
        None => "position unknown".to_string(),
    };
    format!("{} {} ch {} ({at})", ostr(&t.codec), ostr(&t.channels), ostr(&t.language))
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// {(slug, label)} of every rip whose rip-summary.json or extract-summary.json says "ok": true. Py: ripped.
fn ripped(tree: &dyn Tree) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for fname in ["rip-summary.json", "extract-summary.json"] {
        for path in tree.glob(&PathKey::root(), &format!("*/*/{fname}")) {
            if let Ok(data) = read_json::<Value>(tree, &path) {
                if data.get("ok") == Some(&Value::Bool(true)) {
                    out.insert((path.components()[0].clone(), path.components()[1].clone()));
                }
            }
        }
    }
    out
}

/// True when the rip's ffprobe-streams.json lists streams and none of them is audio. Py: no_audio.
fn no_audio(tree: &dyn Tree, rip_dir: &PathKey) -> bool {
    let Ok(data) = read_json::<Value>(tree, &rip_dir.join("ffprobe-streams.json")) else { return false };
    match data.get("streams") {
        Some(Value::Array(streams)) => {
            !streams.is_empty()
                && !streams.iter().any(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"))
        }
        _ => false,
    }
}

/// The report's lines, the reasons for exit 1, and the labels without a measurement. Py: report.
struct ReportOut {
    lines: Vec<String>,
    failures: Vec<String>,
    orphans: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
fn report(
    tree: &dyn Tree,
    pass: &Pass,
    labelled: &BTreeMap<String, Labels>,
    req: &Requirements,
    titles: &BTreeMap<(String, String), TitleFacts>,
    shown: &Ordered<Num>,
    proxies: &BTreeSet<(String, String)>,
    rips: &BTreeSet<(String, String)>,
) -> ReportOut {
    let tracks = &pass.tracks;
    let good = judged(tracks);
    let cands: Vec<&Track> = good.iter().copied().filter(|t| t.role == Role::Candidate).collect();
    let refs = good.iter().filter(|t| t.role == Role::Reference).count();
    let folders: BTreeSet<(&str, &str, &str)> =
        tracks.iter().map(|t| (t.slug.as_str(), t.label.as_str(), t.folder.as_str())).collect();
    let wrong: BTreeSet<(&str, &str, &str, &str)> = tracks
        .iter()
        .filter(|t| t.excluded)
        .map(|t| (t.slug.as_str(), t.label.as_str(), t.folder.as_str(), t.ref_problem.as_deref().unwrap_or("")))
        .collect();
    let discs: BTreeSet<&str> = tracks.iter().map(|t| t.slug.as_str()).collect();
    let title_count = tracks.iter().map(|t| (t.slug.as_str(), t.label.as_str())).collect::<BTreeSet<_>>().len();
    let labelled_cands = cands.iter().filter(|t| t.labelled()).count();
    let rule_text = if shown.is_empty() {
        "thresholds and constants as in the code".to_string()
    } else {
        shown.iter().map(|(key, value)| format!("{} {value}", key.to_uppercase())).collect::<Vec<_>>().join(", ")
    };
    let mut lines = vec![
        format!(
            "audio dataset: {CORPUS}: {}, {}, {} ({} measured against the wrong reference), {} ({} labelled), {}",
            plural(discs.len(), "disc"),
            plural(title_count, "title"),
            plural(folders.len(), "level folder"),
            wrong.len(),
            plural(cands.len(), "measured track"),
            labelled_cands,
            plural(refs, "reference")
        ),
        // spec 3.4: the Python printed "rule: audio_similarity.py <sha>, ..."
        format!("rule: version {RULE_VERSION}, {rule_text}"),
        format!(
            "measures: {} analysed now, {} taken from the cache (the snapshot, or this run's baseline)",
            pass.computed, pass.reused
        ),
        String::new(),
    ];
    let todo: Vec<&Track> = cands.iter().copied().filter(|t| t.kind.is_none()).collect();
    lines.push("To label (measured, no label yet; the owner answers before seeing any verdict):".to_string());
    for t in &todo {
        let disagree = t.ambiguous.as_ref().map(|a| format!("; {a}: they disagree")).unwrap_or_default();
        lines.push(format!("  {}: {}{disagree}", name(t), describe(t)));
        // spec 3.4: the Python printed "python3 scripts/phase0/audio_labels.py set ..."
        lines.push(format!(
            "    lazymkv-dev labels set corpus/owner/{} {} {} <kind> --how <how> --saw-verdict no",
            t.slug, t.label, t.stream
        ));
    }
    if todo.is_empty() {
        lines.push("  none".to_string());
    }
    lines.push(String::new());
    lines.push("Kind against verdict (labelled candidates; references are not judged):".to_string());
    lines.extend(table(tracks));
    let (c, answer, false_tracks) = coverage(tracks, req, titles);
    let mut not_counted: BTreeMap<&str, usize> = BTreeMap::new();
    for t in &cands {
        if !t.counts {
            *not_counted.entry(t.slug.as_str()).or_insert(0) += 1;
        }
    }
    let of = |n: Option<i64>| n.map(|n| format!(" of {n}")).unwrap_or_default();
    let dubs_why = if req.dubs.is_some() {
        "what row 13 asks before content may judge a candidate in another language"
    } else {
        "row 13 sets no number for dubs yet"
    };
    let not_counted_text: Vec<String> = not_counted.iter().map(|(s, n)| format!("{s} {n}")).collect();
    lines.push(String::new());
    lines.push(format!(
        "Experiment 13 (spec section 17 row 13; tracks that count: content checked by listening before the verdict, the owner sure, each disc's packets once, never {} or a disc or track in audio-fitted.json; the packaging, the menu, a flag and TheDiscDB are observations and do not count on their own, D23):",
        req.fitting_slug
    ));
    lines.push(format!(
        "  commentaries          {} of {}, from {} of {} discs; on feature films {} of {}; film clearly audible {} of {} (of them partial: {})",
        c.commentaries, req.commentaries, c.commentary_discs, req.commentary_discs, c.commentary_features,
        req.commentary_features, c.commentary_audible, req.commentary_audible, c.partial
    ));
    lines.push(format!(
        "  audio description     {} ({} needed if the owner has a disc with one, up to {}; checked by hand)",
        c.ad, req.ad_min, req.ad_max
    ));
    lines.push(format!(
        "  programme tracks      {} of {} other than the reference; separately encoded {} of {}; derived cores {} (row 13 does not exclude them from the {})",
        c.programme, req.programme, c.programme_separate, req.programme_separate, c.derived_cores, req.programme
    ));
    lines.push(format!(
        "  no centre channel     {} of {} titles with a counted extra track",
        c.no_centre_titles, req.no_centre_titles
    ));
    lines.push(format!(
        "  dialogue-heavy        {} of {} titles with a counted extra track",
        c.dialogue_heavy_titles, req.dialogue_heavy_titles
    ));
    lines.push(format!(
        "  score, M&E, dub, narration  isolated-score {}, music-effects {}, same-language-dub {}, alternate-narration {} (every one the owner has; checked by hand)",
        c.others[0], c.others[1], c.others[2], c.others[3]
    ));
    lines.push(format!("  commentary verdicts   on {} of {} discs", c.verdict_discs, req.verdict_discs));
    lines.push(format!(
        "  cross-language        commentaries {} (commentary-cross-language {}), programme tracks {}, judged against a reference in another language; never dropped by content (addendum 9.2)",
        c.cross_comm, c.cross_comm_pattern, c.cross_prog
    ));
    lines.push(format!(
        "  dubs                  {}{} from {}{} discs; on dialogue-heavy titles {}{}; with the commentary pattern {} ({dubs_why}; until then such a candidate is never dropped by content)",
        c.dubs, of(req.dubs), c.dub_discs, of(req.dub_discs), c.dub_heavy, of(req.dub_heavy), c.dub_pattern
    ));
    lines.push(format!(
        "  swapped references    {} labelled candidates read the other way round; the programme would get the commentary verdict against {} of them (addendum 9.3; no number in row 13)",
        c.swapped, c.swap_fails
    ));
    lines.push(format!(
        "  false verdicts        {} (a programme track with the commentary pattern: commentary or commentary-cross-language, on any disc)",
        false_tracks.len()
    ));
    lines.push(format!("  as it stands: {answer}"));
    lines.push(format!(
        "  not counted: {}",
        if not_counted_text.is_empty() { "none".to_string() } else { not_counted_text.join(", ") }
    ));
    lines.push(String::new());
    lines.push("Disagreements (labelled candidates):".to_string());
    let start = lines.len();
    let mut failures = Vec::new();
    let fails: Vec<&Track> = tracks.iter().filter(|t| t.outcome == "FAIL").collect();
    for t in &fails {
        let from = t.inherited_from.as_ref().map(|f| format!(" (label of {f})")).unwrap_or_default();
        lines.push(format!(
            "  FAIL {}: labelled {} (programme audio), verdict {}{from}",
            where_of(t),
            ostr(&t.kind),
            vstr(t.verdict)
        ));
    }
    if !fails.is_empty() {
        let names: Vec<String> = fails.iter().map(|t| where_of(t)).collect();
        failures.push(format!(
            "{} programme track(s) got a drop verdict or the commentary pattern: {}",
            fails.len(),
            names.join("; ")
        ));
    }
    for (s, l, f, p) in &wrong {
        // spec 3.4: the Python named `audio_dataset.py args`
        lines.push(format!(
            "  FAIL {s}/{l}/{f}: wrong reference: {p}; its tracks are left out (measure the title again with the reference that `lazymkv-dev levels-args` prints, then delete this folder)"
        ));
    }
    if !wrong.is_empty() {
        let names: Vec<String> = wrong.iter().map(|(s, l, f, _)| format!("{s}/{l}/{f}")).collect();
        failures.push(format!("{} folder(s) measured against the wrong reference: {}", wrong.len(), names.join("; ")));
    }
    let swaps: Vec<&Track> = tracks.iter().filter(|t| t.swap_fail).collect();
    for t in &swaps {
        lines.push(format!(
            "  FAIL {}: swapped reference: labelled {}, verdict {}; with this track as the reference, the programme (stream {}) would get the commentary verdict (EXP-13 addendum, section 9.3)",
            where_of(t),
            ostr(&t.kind),
            vstr(t.verdict),
            t.main
        ));
    }
    if !swaps.is_empty() {
        let names: Vec<String> = swaps.iter().map(|t| where_of(t)).collect();
        failures.push(format!("{} swapped reference(s) would drop the programme: {}", swaps.len(), names.join("; ")));
    }
    let bad_refs: Vec<&Track> = tracks.iter().filter(|t| t.outcome == "reference is unwanted audio").collect();
    for t in &bad_refs {
        lines.push(format!(
            "  FAIL {}: the reference is labelled {} (unwanted audio): its folder measures the title against it",
            where_of(t),
            ostr(&t.kind)
        ));
    }
    if !bad_refs.is_empty() {
        let names: Vec<String> = bad_refs.iter().map(|t| where_of(t)).collect();
        failures.push(format!("{} reference(s) labelled as unwanted audio: {}", bad_refs.len(), names.join("; ")));
    }
    let kept: Vec<&Track> = cands.iter().copied().filter(|t| t.outcome == "kept").collect();
    for &(kind, _, _, _) in KINDS.iter() {
        let rows: Vec<String> = kept
            .iter()
            .filter(|t| kind_is(t, kind))
            .map(|t| {
                let nj = if t.not_judged.is_empty() { String::new() } else { format!(", not judged: {}", t.not_judged) };
                format!("{} ({}{nj})", name(t), vstr(t.verdict))
            })
            .collect();
        if !rows.is_empty() {
            lines.push(format!("  kept although unwanted, {kind}: {}", rows.join("; ")));
        }
    }
    for t in tracks.iter().filter(|t| !t.excluded) {
        if t.outcome == "dropped as a commentary"
            || t.outcome.starts_with("undecided kind, ")
            || t.outcome.starts_with("unknown kind, ")
            || t.outcome.starts_with("reference labelled")
        {
            lines.push(format!("  note: {}: {} ({}, verdict {})", where_of(t), t.outcome, ostr(&t.kind), vstr(t.verdict)));
        }
    }
    for t in &good {
        if t.role == Role::Reference && !t.veto.is_empty() {
            lines.push(format!(
                "  note: {}/{}/{}: the position veto holds (row 0, D21): {}; no commentary is confirmed on its candidates: each is reference-unclear",
                t.slug, t.label, t.folder, t.veto
            ));
        }
    }
    let mut seen: BTreeSet<(&str, &str, i64, Role)> = BTreeSet::new();
    for t in &good {
        if !t.also_in.is_empty() && seen.insert((t.slug.as_str(), t.label.as_str(), t.stream, t.role)) {
            let same: Vec<&Track> = tracks
                .iter()
                .filter(|o| o.slug == t.slug && o.label == t.label && o.stream == t.stream && o.role == t.role && !o.excluded)
                .collect();
            let verdicts: BTreeSet<Option<Verdict>> = same.iter().map(|o| o.verdict).collect();
            let measured_in: Vec<&str> = same.iter().map(|o| o.folder.as_str()).collect();
            let mut line = format!(
                "  note: {} ({}) is measured in {}; the table uses {}",
                name(t),
                t.role.as_str(),
                measured_in.join(", "),
                t.folder
            );
            if verdicts.len() > 1 {
                line.push_str("; their verdicts differ");
                if t.labelled() {
                    let each: Vec<String> = same.iter().map(|o| format!("{} {}", o.folder, vstr(o.verdict))).collect();
                    line.push_str(&format!(": {}", each.join(", ")));
                }
            }
            lines.push(line);
        }
    }
    for t in tracks {
        if t.ref_problem.is_none() && t.role == Role::Reference {
            // spec 3.4: the Python named proxy_levels.py
            lines.push(format!(
                "  note: {}/{}/{}: no streams.json, so neither its reference nor the position veto (row 0, D21) is checked: none of its candidates counts for experiment 13 (lazymkv-dev levels-from-proxy writes streams.json from audio-proxy.json, or copy the same rip's ffprobe-streams.json there as streams.json)",
                t.slug, t.label, t.folder
            ));
        }
    }
    for t in &cands {
        if t.labelled() && !t.unjudged.is_empty() {
            lines.push(format!(
                "  note: {}: {}; labelled {}, verdict {}: not counted for experiment 13 and never a FAIL",
                where_of(t),
                t.unjudged,
                ostr(&t.kind),
                vstr(t.verdict)
            ));
        }
    }
    for t in &cands {
        if let Some(from) = &t.inherited_from {
            lines.push(format!("  note: {} takes its label from {from} (same packets)", name(t)));
        }
    }
    if lines.len() == start {
        lines.push("  none".to_string());
    }
    let have: BTreeSet<(&str, &str, i64)> = good.iter().map(|t| (t.slug.as_str(), t.label.as_str(), t.stream)).collect();
    let mut orphans = Vec::new();
    for (slug, data) in labelled {
        for t in &data.tracks {
            if !have.contains(&(slug.as_str(), t.label.as_str(), t.stream)) {
                orphans.push(format!("{slug}/{} stream {}: {}", t.label, t.stream, t.kind));
            }
        }
    }
    lines.push(String::new());
    lines.push("Labels without a measurement:".to_string());
    if orphans.is_empty() {
        lines.push("  none".to_string());
    }
    lines.extend(orphans.iter().map(|o| format!("  {o}")));
    lines.push(String::new());
    lines.push("Folders that cannot be read:".to_string());
    if pass.unreadable.is_empty() {
        lines.push("  none".to_string());
    }
    lines.extend(pass.unreadable.iter().map(|u| format!("  {u}")));
    let mut measured: BTreeSet<(String, String)> = tracks.iter().map(|t| (t.slug.clone(), t.label.clone())).collect();
    for u in &pass.unreadable {
        let parts: Vec<&str> = u.split('/').collect();
        measured.insert((parts[0].to_string(), parts.get(1).copied().unwrap_or("").to_string()));
    }
    let mut titles_seen: Vec<(String, String)> = measured.union(rips).cloned().collect();
    let silent: Vec<(String, String)> = titles_seen
        .iter()
        .filter(|x| !measured.contains(*x) && !proxies.contains(*x) && no_audio(tree, &PathKey::new(&x.0).join(&x.1)))
        .cloned()
        .collect();
    titles_seen.retain(|x| !silent.contains(x));
    let missing: Vec<String> = titles_seen
        .iter()
        .filter(|x| !proxies.contains(*x))
        .map(|x| format!("{}/{}{}", x.0, x.1, if measured.contains(x) { "" } else { " (not measured)" }))
        .collect();
    lines.push(String::new());
    lines.push(format!(
        "Titles whose audio is in the tuning dataset (audio-proxy.json): {} of {} (every successful rip and every measured title){}",
        titles_seen.len() - missing.len(),
        titles_seen.len(),
        if missing.is_empty() { String::new() } else { format!("; missing: {}", missing.join(", ")) }
    ));
    if !silent.is_empty() {
        let names: Vec<String> = silent.iter().map(|(s, l)| format!("{s}/{l}")).collect();
        lines.push(format!(
            "  note: no audio stream (its ffprobe-streams.json lists none), so nothing to add and not missing: {}",
            names.join(", ")
        ));
    }
    ReportOut { lines, failures, orphans }
}

/// {track key: (verdict, kind, outcome)}. Py: verdicts_of.
type Verdicts = BTreeMap<String, (Option<String>, Option<String>, Option<String>)>;

/// verdicts_of(tracks) for this run's tracks.
fn run_verdicts(tracks: &[Track]) -> Verdicts {
    tracks
        .iter()
        .map(|t| (t.key(), (t.verdict.map(|v| v.as_str().to_string()), t.kind.clone(), Some(t.outcome.clone()))))
        .collect()
}

/// A JSON value as the f-string prints it; None for an absent key or null.
fn text_of(v: Option<&Value>) -> Option<String> {
    match v {
        None | Some(Value::Null) => None,
        Some(v) => Some(py_str(v)),
    }
}

/// verdicts_of(snapshot["tracks"], snapshot.get("unlabelled_verdicts")).
fn snapshot_verdicts(s: &PreviousSnapshot) -> Verdicts {
    let mut out = BTreeMap::new();
    for t in &s.tracks {
        let part = |k: &str| t.get(k).map_or_else(|| "None".to_string(), py_str);
        let stream = t.get("stream").map_or_else(|| "None".to_string(), py_str);
        let key = format!("{}/{}/{}/{}/{stream}", part("slug"), part("label"), part("folder"), part("role"));
        let mut v = text_of(t.get("verdict"));
        if v.is_none() && !s.unlabelled_verdicts.is_empty() {
            v = text_of(s.unlabelled_verdicts.get(&key));
        }
        out.insert(key, (v, text_of(t.get("kind")), text_of(t.get("outcome"))));
    }
    out
}

/// Lines for every candidate whose verdict differs between two runs, and for the tracks only one of them has; an
/// unlabelled track's verdict is never shown. Returns the block (title first) and the keys gone. Py: changes.
fn changes(old: &Verdicts, new: &Verdicts, title: &str) -> (Vec<String>, Vec<String>) {
    let mut out = Vec::new();
    let mut hidden = 0;
    for (k, (v, kind, outcome)) in new {
        if !k.contains("/candidate/") {
            continue;
        }
        let Some(before) = old.get(k) else { continue };
        if before.0 == *v {
            continue;
        }
        let Some(kind) = kind else {
            hidden += 1;
            continue;
        };
        let parts: Vec<&str> = k.split('/').collect();
        let [slug, label, folder, _, stream] = parts.as_slice() else { continue };
        let fail = if outcome.as_deref() == Some("FAIL") { "   FAIL" } else { "" };
        out.push(format!(
            "  {slug}/{label} stream {stream} in {folder} ({kind}): {} -> {}{fail}",
            before.0.as_deref().unwrap_or("None"),
            v.as_deref().unwrap_or("None")
        ));
    }
    if hidden > 0 {
        out.push(format!("  {hidden} unlabelled track(s) changed verdict (not shown until labelled)"));
    }
    let gone: Vec<String> = old.keys().filter(|k| !new.contains_key(*k)).cloned().collect();
    let added: Vec<String> = new.keys().filter(|k| !old.contains_key(*k)).cloned().collect();
    if !gone.is_empty() {
        out.push(format!("  gone: {} track(s) not measured now: {}", gone.len(), gone.join(", ")));
    }
    if !added.is_empty() {
        out.push(format!("  new: {} track(s): {}", added.len(), added.join(", ")));
    }
    if out.is_empty() {
        out.push("  none".to_string());
    }
    let mut block = vec![title.to_string()];
    block.extend(out);
    (block, gone)
}

/// The snapshot of the run (the trial's when there are overrides). Py: write_snapshot's dict.
fn build_snapshot(input: &HarnessInput, pass: &Pass, rule: Rule, consts: Constants, fitted: &Fitted) -> Snapshot {
    let mut rows = Vec::new();
    let mut hidden: Ordered<Verdict> = Ordered::new();
    for t in &pass.tracks {
        let mut row = SnapshotTrack {
            slug: t.slug.clone(),
            label: t.label.clone(),
            folder: t.folder.clone(),
            role: t.role.as_str().to_string(),
            stream: t.stream,
            main: t.main,
            md5: t.md5.clone(),
            md5_rank: t.md5_rank,
            codec: t.codec.clone(),
            channels: t.channels.clone(),
            language: t.language.clone(),
            ref_fc: t.ref_fc,
            ref_problem: t.ref_problem.clone(),
            excluded: t.excluded,
            secondary: t.secondary,
            verdict: t.verdict,
            not_judged: t.not_judged.clone(),
            duplicate_of: t.duplicate_of.clone(),
            measures: t.measures.clone(),
            key: t.key.clone(),
            kind: t.kind.clone(),
            inherited_from: t.inherited_from.clone(),
            counts: t.counts,
            why_not_counted: t.why_not_counted.clone(),
            outcome: t.outcome.clone(),
            cross_language: t.cross_language.clone(),
            swapped_verdict: t.swapped_verdict,
            swap_fail: t.swap_fail,
            peer: t.peer.clone(),
            ref_first: t.ref_first.clone(),
            veto: t.veto.clone(),
            before_veto: t.before_veto,
            owner_label: t.label_entry.as_ref().map(|e| e.raw.clone()),
        };
        if t.role == Role::Candidate && t.kind.is_none() {
            if let Some(v) = t.verdict {
                hidden.insert(t.key(), v);
            }
            row.verdict = None;
            row.swapped_verdict = None;
            row.before_veto = None;
            if let Some(peer) = row.peer.as_mut() {
                peer.verdict = None;
            }
        }
        rows.push(row);
    }
    let mut fitted_rows: Vec<(String, FittedKey)> = fitted
        .iter()
        .map(|f| {
            let as_list = Value::Array(vec![
                Value::String(f.0.clone()),
                f.1.clone().map_or(Value::Null, Value::String),
                f.2.map_or(Value::Null, Value::from),
            ]);
            (py_repr(&as_list), f.clone())
        })
        .collect();
    fitted_rows.sort_by(|a, b| a.0.cmp(&b.0)); // sorted([list(f) for f in fitted], key=str)
    Snapshot {
        format: SNAPSHOT_FORMAT.to_string(),
        created: input.created.to_string(),
        corpus: CORPUS.to_string(),
        code: SnapshotCode { rule_version: RULE_VERSION },
        rule,
        constants: consts,
        overrides: SnapshotOverrides {
            rule: input.overrides.rule.clone(),
            constants: input.overrides.constants.clone(),
        },
        spec_row13: input.req.clone(),
        fitted: fitted_rows.into_iter().map(|(_, f)| f).collect(),
        tracks: rows,
        unlabelled_verdicts_note: UNLABELLED_NOTE.to_string(),
        unlabelled_verdicts: hidden,
        unreadable: pass.unreadable.clone(),
        measures_cache: pass.used.clone(),
    }
}

#[cfg(test)]
mod tests;
