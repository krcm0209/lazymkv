//! The audio content rule of spec 8.6 (EXP-13-design.md sections 3 and 9; owner decision D21): the decision
//! thresholds, the verdict of one measured pair, the position veto (row 0) and the verdicts of a whole level folder.
//! Ported from scripts/phase0/audio_similarity.py: RULE, EARLIER_REVERSE, commentary_pattern, ad_pattern, not_judged,
//! reverse_pattern, verdict, MEANING and VETO_MEANING first; first_of_language, position_veto, cross_language and
//! results after them. Stereo width, codec and track position never decide what a track is (D15): position only
//! withholds a drop, through the veto.

use std::collections::BTreeMap;

use crate::audio::levels::{read_levels, read_manifest, refusal, Fields, Manifest};
use crate::audio::measures::{
    analyse, ok, py_max, reverse_measures, Constants, Measures, Num, NumKind, Reverse, Run, SetError,
};
use crate::audio::reference::{ffprobe_audio, int_text, manifest_stream, placed_first, read_streams, AudioStream};
use crate::lang::NO_LANGUAGE;
use crate::recording::files::FfprobeStream;
use crate::recording::{py_repr_str, PathKey, Tree};

/// The rule's version, in the harness's cache key and the snapshot's "code"; bumped by hand with every change to a
/// measure, a constant's default or a threshold (spec 3.3, RP5). Replaces the Python's SHA-1 of audio_similarity.py.
pub const RULE_VERSION: u32 = 1;

/// The decision thresholds, with the Python's defaults (every one PROVISIONAL: EXP-13-design.md, section 3); fields
/// in RULE's order, so the serde form is the snapshot's "rule" (tenths_min an int, the others floats). Py: RULE.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    pub overlap_min: f64,
    pub main_silent_max: f64,
    pub gap_s_min: f64,
    pub dc_max: f64,
    pub ag_min: f64,
    pub tenths_min: i64,
    pub tenths_share: f64,
    pub smi_min: f64,
    pub ls_max: f64,
    pub added_s_min: f64,
    pub ad_dc_min: f64,
    pub ad_adl_max: f64,
    pub ad_ag_min: f64,
    pub ad_ag_s_min: f64,
    pub ad_tenths_share: f64,
    pub note_comm_dc_max: f64,
    pub note_comm_ag_min: f64,
    pub note_ad_dc_min: f64,
    pub note_ad_ag_min: f64,
    pub note_ad_ag_s_min: f64,
}

impl Default for Rule {
    /// RULE as audio_similarity.py writes it.
    fn default() -> Rule {
        Rule {
            overlap_min: 0.90,
            main_silent_max: 0.10,
            gap_s_min: 90.0,
            dc_max: 0.30,
            ag_min: 0.50,
            tenths_min: 6,
            tenths_share: 0.90,
            smi_min: 0.50,
            ls_max: 13.0,
            added_s_min: 60.0,
            ad_dc_min: 0.90,
            ad_adl_max: 0.02,
            ad_ag_min: 0.15,
            ad_ag_s_min: 45.0,
            ad_tenths_share: 0.60,
            note_comm_dc_max: 0.50,
            note_comm_ag_min: 0.30,
            note_ad_dc_min: 0.80,
            note_ad_ag_min: 0.10,
            note_ad_ag_s_min: 30.0,
        }
    }
}

impl Rule {
    /// The 20 keys in RULE's order, each with its kind (only tenths_min is Int).
    pub const NAMES: [(&'static str, NumKind); 20] = [
        ("overlap_min", NumKind::Float),
        ("main_silent_max", NumKind::Float),
        ("gap_s_min", NumKind::Float),
        ("dc_max", NumKind::Float),
        ("ag_min", NumKind::Float),
        ("tenths_min", NumKind::Int),
        ("tenths_share", NumKind::Float),
        ("smi_min", NumKind::Float),
        ("ls_max", NumKind::Float),
        ("added_s_min", NumKind::Float),
        ("ad_dc_min", NumKind::Float),
        ("ad_adl_max", NumKind::Float),
        ("ad_ag_min", NumKind::Float),
        ("ad_ag_s_min", NumKind::Float),
        ("ad_tenths_share", NumKind::Float),
        ("note_comm_dc_max", NumKind::Float),
        ("note_comm_ag_min", NumKind::Float),
        ("note_ad_dc_min", NumKind::Float),
        ("note_ad_ag_min", NumKind::Float),
        ("note_ad_ag_s_min", NumKind::Float),
    ];

    /// The value of a key exactly as NAMES spells it; None for any other name. Py: RULE[key].
    pub fn get(&self, key: &str) -> Option<Num> {
        let value = match key {
            "tenths_min" => return Some(Num::Int(self.tenths_min)),
            "overlap_min" => self.overlap_min,
            "main_silent_max" => self.main_silent_max,
            "gap_s_min" => self.gap_s_min,
            "dc_max" => self.dc_max,
            "ag_min" => self.ag_min,
            "tenths_share" => self.tenths_share,
            "smi_min" => self.smi_min,
            "ls_max" => self.ls_max,
            "added_s_min" => self.added_s_min,
            "ad_dc_min" => self.ad_dc_min,
            "ad_adl_max" => self.ad_adl_max,
            "ad_ag_min" => self.ad_ag_min,
            "ad_ag_s_min" => self.ad_ag_s_min,
            "ad_tenths_share" => self.ad_tenths_share,
            "note_comm_dc_max" => self.note_comm_dc_max,
            "note_comm_ag_min" => self.note_comm_ag_min,
            "note_ad_dc_min" => self.note_ad_dc_min,
            "note_ad_ag_min" => self.note_ad_ag_min,
            "note_ad_ag_s_min" => self.note_ad_ag_s_min,
            _ => return None,
        };
        Some(Num::Float(value))
    }

    /// Sets a key as `--set NAME=VALUE` does for a rule threshold (Py: parse_sets' rule branch) and returns the value
    /// stored. The name is matched case-insensitively: it is lower-cased here as parse_sets lower-cases it (the caller
    /// may lower-case it first). tenths_min takes a whole number only and stores it as an int (NotWhole otherwise,
    /// the rule unchanged); every other key keeps the value as a float. Unknown for a name that is not a key of RULE
    /// (parse_sets then tries the measure constants). The value is finite: parse_sets refuses a non-finite one, with
    /// the Python's message, before it calls this; a non-finite value for tenths_min is NotWhole here. The messages
    /// that list the valid names belong to parse_sets (Task 8), which has the item as the user typed it.
    pub fn set(&mut self, key: &str, value: f64) -> Result<Num, SetError> {
        let key = key.to_lowercase();
        let Some(&(_, kind)) = Rule::NAMES.iter().find(|(name, _)| *name == key) else {
            return Err(SetError::Unknown);
        };
        match kind {
            NumKind::Int => {
                // Python: `if value != int(value)`; tenths_min is the only Int key of NAMES.
                if !value.is_finite() || value != value.trunc() {
                    return Err(SetError::NotWhole);
                }
                self.tenths_min = value as i64;
                Ok(Num::Int(self.tenths_min))
            }
            NumKind::Float => {
                let slot = self.float_slot(&key).ok_or(SetError::Unknown)?;
                *slot = value;
                Ok(Num::Float(value))
            }
        }
    }

    /// The f64 field a Float key of NAMES names.
    fn float_slot(&mut self, key: &str) -> Option<&mut f64> {
        Some(match key {
            "overlap_min" => &mut self.overlap_min,
            "main_silent_max" => &mut self.main_silent_max,
            "gap_s_min" => &mut self.gap_s_min,
            "dc_max" => &mut self.dc_max,
            "ag_min" => &mut self.ag_min,
            "tenths_share" => &mut self.tenths_share,
            "smi_min" => &mut self.smi_min,
            "ls_max" => &mut self.ls_max,
            "added_s_min" => &mut self.added_s_min,
            "ad_dc_min" => &mut self.ad_dc_min,
            "ad_adl_max" => &mut self.ad_adl_max,
            "ad_ag_min" => &mut self.ad_ag_min,
            "ad_ag_s_min" => &mut self.ad_ag_s_min,
            "ad_tenths_share" => &mut self.ad_tenths_share,
            "note_comm_dc_max" => &mut self.note_comm_dc_max,
            "note_comm_ag_min" => &mut self.note_comm_ag_min,
            "note_ad_dc_min" => &mut self.note_ad_dc_min,
            "note_ad_ag_min" => &mut self.note_ad_ag_min,
            "note_ad_ag_s_min" => &mut self.note_ad_ag_s_min,
            _ => return None,
        })
    }
}

/// The earlier reverse test (EXP-13-design.md section 2 item 13: 90 s of the candidate's gaps, reverse AG 0.30). It
/// is replaced by the reverse commentary pattern (addendum 9.3) and used only for measures that carry rev_gap_s and
/// rev_ag alone (Reverse::Earlier, as the code before 2026-10-06 wrote them): it can only keep. Py: EARLIER_REVERSE.
pub const EARLIER_REVERSE_GAP_S_MIN: f64 = 90.0;
pub const EARLIER_REVERSE_AG_MIN: f64 = 0.30;

/// A candidate's verdict; `as_str` and serde give the Python's strings; ALL is audio_dataset.VERDICTS' order.
/// Py: the verdict strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Duplicate,
    Commentary,
    CommentaryCrossLanguage,
    ReferenceUnclear,
    AdPattern,
    NoteCommentary,
    NoteAd,
    Keep,
    NotJudged,
}

impl Verdict {
    /// duplicate, commentary, commentary-cross-language, reference-unclear, ad-pattern, note-commentary, note-ad,
    /// keep, not-judged. Py: audio_dataset.VERDICTS.
    pub const ALL: [Verdict; 9] = [
        Verdict::Duplicate,
        Verdict::Commentary,
        Verdict::CommentaryCrossLanguage,
        Verdict::ReferenceUnclear,
        Verdict::AdPattern,
        Verdict::NoteCommentary,
        Verdict::NoteAd,
        Verdict::Keep,
        Verdict::NotJudged,
    ];

    /// The Python's string.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Duplicate => "duplicate",
            Verdict::Commentary => "commentary",
            Verdict::CommentaryCrossLanguage => "commentary-cross-language",
            Verdict::ReferenceUnclear => "reference-unclear",
            Verdict::AdPattern => "ad-pattern",
            Verdict::NoteCommentary => "note-commentary",
            Verdict::NoteAd => "note-ad",
            Verdict::Keep => "keep",
            Verdict::NotJudged => "not-judged",
        }
    }
}

impl std::str::FromStr for Verdict {
    type Err = String;

    /// Only the Python's spelling of a verdict; Err "'<text>' is not a verdict" otherwise.
    fn from_str(text: &str) -> Result<Verdict, String> {
        Verdict::ALL
            .into_iter()
            .find(|v| v.as_str() == text)
            .ok_or_else(|| format!("{} is not a verdict", py_repr_str(text)))
    }
}

/// The report's meaning of a verdict, verbatim. Py: MEANING.
pub fn meaning(v: Verdict) -> &'static str {
    match v {
        Verdict::Duplicate => "exact duplicate (same packets): keep the first copy, drop this one; no audio analysis needed",
        Verdict::Commentary => "commentary pattern: kept and flagged for review until experiment 13 answers yes, then dropped",
        Verdict::CommentaryCrossLanguage => "commentary pattern on a track in another language than the reference: keep and flag for review (commentary_detected); never dropped by content until real dubs in a wanted language have been measured (EXP-13 addendum 9.2)",
        Verdict::ReferenceUnclear => "the main track looks like a commentary over this one (reverse run): one of the two is not what its rank says (a commentary or a stem ranked first, or a stem among the candidates): keep, review",
        Verdict::AdPattern => "audio-description pattern: keep, note possible_audio_description (never dropped by content)",
        Verdict::NoteCommentary => "keep, note possible_commentary",
        Verdict::NoteAd => "keep, note possible_audio_description",
        Verdict::Keep => "keep: programme audio, nothing added",
        Verdict::NotJudged => "keep: no content verdict",
    }
}

/// The veto's meaning, verbatim. Py: VETO_MEANING.
pub const VETO_MEANING: &str = "the position veto holds (row 0, D21): the reference may not be the programme, so no commentary is confirmed on this title: keep, review (audio_reference_unclear)";

/// The commentary pattern of the forward run or of a reverse run (rows 3 and 4). Py: commentary_pattern.
pub fn commentary_pattern(m: &Run, rule: &Rule) -> bool {
    ok(m.dc)
        && m.dc <= rule.dc_max
        && ok(m.ag)
        && m.ag >= rule.ag_min
        && m.tenths >= rule.tenths_min
        && (m.tenths_comm as f64) >= rule.tenths_share * (m.tenths as f64)
        && ok(m.smi)
        && m.smi >= rule.smi_min
        && m.ls_db <= rule.ls_max
        && m.added_s >= rule.added_s_min
}

/// The audio-description pattern (row 5). Py: ad_pattern.
pub fn ad_pattern(m: &Run, rule: &Rule) -> bool {
    ok(m.dc)
        && m.dc >= rule.ad_dc_min
        && ok(m.adl)
        && m.adl <= rule.ad_adl_max
        && ok(m.ag)
        && m.ag >= rule.ad_ag_min
        && m.ag_s >= rule.ad_ag_s_min
        && m.tenths >= rule.tenths_min
        && (m.tenths_ad as f64) >= rule.ad_tenths_share * (m.tenths as f64)
        && ok(m.smi)
        && m.smi >= rule.smi_min
}

/// Why the content rule may not confirm anything for this pair (the error, or one of the four sentences), "" when it
/// may. Py: not_judged.
pub fn not_judged(m: &Measures, rule: &Rule) -> String {
    let run = match m {
        Measures::Error { error, .. } => return error.clone(),
        Measures::Run(run) => run,
    };
    if run.overlap < rule.overlap_min {
        return "the tracks overlap for less than 90% of the main track".to_string();
    }
    if run.main_silent > rule.main_silent_max {
        return "the main track is digitally silent for more than 10% of the time (not continuous programme sound)".to_string();
    }
    if run.gap_s < rule.gap_s_min {
        return "the main track has less than 90 s of dialogue gaps".to_string();
    }
    if !ok(run.dc) {
        return "the main track has no dialogue windows".to_string();
    }
    String::new()
}

/// True when the reverse run may be judged and has the commentary pattern: the reference looks like a commentary
/// over the candidate (EXP-13 addendum 9.3). The same thresholds as the forward pattern. Py: reverse_pattern.
pub fn reverse_pattern(m: &Measures, rule: &Rule) -> bool {
    match reverse_measures(m) {
        Some(r) => match r.run() {
            Some(run) => not_judged(&r, rule).is_empty() && commentary_pattern(run, rule),
            None => false,
        },
        None => false,
    }
}

/// One verdict of rows 3 to 9: commentary, reference-unclear, ad-pattern, note-commentary, note-ad, keep, or
/// not-judged for an error (never duplicate or commentary-cross-language: results gives those). reference-unclear:
/// the reverse run has the commentary pattern, whatever the forward run says; for measures without a reverse block,
/// the earlier reverse test applies. Py: verdict.
pub fn verdict(m: &Measures, rule: &Rule) -> Verdict {
    let run = match m {
        Measures::Error { .. } => return Verdict::NotJudged,
        Measures::Run(run) => run,
    };
    let why = not_judged(m, rule);
    if reverse_measures(m).is_some() {
        if reverse_pattern(m, rule) {
            return Verdict::ReferenceUnclear;
        }
        if why.is_empty() && commentary_pattern(run, rule) {
            return Verdict::Commentary;
        }
    } else if why.is_empty() && commentary_pattern(run, rule) {
        // m.get("rev_ag", nan) and m.get("rev_gap_s", 0.0): absent keys never pass the earlier test.
        let earlier = match run.rev {
            Reverse::Earlier { rev_gap_s, rev_ag } => {
                ok(rev_ag) && rev_gap_s >= EARLIER_REVERSE_GAP_S_MIN && rev_ag >= EARLIER_REVERSE_AG_MIN
            }
            _ => false,
        };
        return if earlier { Verdict::ReferenceUnclear } else { Verdict::Commentary };
    }
    if why.is_empty() && ad_pattern(run, rule) {
        return Verdict::AdPattern;
    }
    if ok(run.dc) && ok(run.ag) && run.dc <= rule.note_comm_dc_max && run.ag >= rule.note_comm_ag_min {
        return Verdict::NoteCommentary;
    }
    if ok(run.dc)
        && ok(run.ag)
        && run.dc >= rule.note_ad_dc_min
        && run.ag >= rule.note_ad_ag_min
        && run.ag_s >= rule.note_ad_ag_s_min
    {
        return Verdict::NoteAd;
    }
    Verdict::Keep
}

/// Row 0 of the rule (EXP-13 addendum 9.5; owner decision D21) for the title whose reference is a given stream:
/// holds is Some(true) when the reference is not the first stream of its language in the file and that first stream
/// is lossless or has more than two channels (the reference choice may be inverted: a score or a commentary ranked
/// above the programme, so the rule confirms no commentary on the title); Some(false) when the reference is that
/// first stream, or that first stream is lossy with two channels or fewer; None when it cannot be checked. why names
/// the streams. Position never decides what a track is (D15): the veto only withholds a drop. Serialised as
/// {"holds": true|false|null, "why": "..."}. Py: position_veto's tuple.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Veto {
    pub holds: Option<bool>,
    pub why: String,
}

/// The first stream of `audio` (file order) in stream `main`'s language when it is not `main` and is lossless or has
/// more than two channels; None otherwise and when `main` is not in `audio`. A lossy first stream of two channels or
/// fewer is what G1 and G2 pass over on purpose, so it raises nothing. Py: audio_similarity.first_of_language.
pub fn first_of_language(audio: &[AudioStream], main: i64) -> Option<AudioStream> {
    let mine = audio.iter().find(|a| a.stream == main)?;
    // min() keeps the first of equal stream numbers, and so does min_by_key.
    let first = audio.iter().filter(|a| a.language == mine.language).min_by_key(|a| a.stream)?;
    if first.stream == main || !(first.lossless || first.channels > 2) {
        None
    } else {
        Some(first.clone())
    }
}

/// Row 0 for the title whose reference is stream `main`; `streams` None means the folder has no streams.json.
/// Py: position_veto, its sentences verbatim.
pub fn position_veto(streams: Option<&[FfprobeStream]>, main: i64) -> Veto {
    let Some(streams) = streams else {
        return Veto { holds: None, why: "not checked: the folder has no streams.json".to_string() };
    };
    let audio = ffprobe_audio(streams);
    let Some(reference) = audio.iter().find(|a| a.stream == main) else {
        return Veto { holds: None, why: format!("not checked: stream {main} is not an audio stream of streams.json") };
    };
    let head = format!("the reference, stream {main} ({}),", reference.description);
    let language = &reference.language;
    // min() keeps the first of equal stream numbers; the reference itself is one of these streams.
    let first = audio.iter().filter(|a| a.language == *language).min_by_key(|a| a.stream).unwrap_or(reference);
    if first.stream == main {
        return Veto { holds: Some(false), why: format!("{head} is the first {language} stream of the file") };
    }
    if first_of_language(&audio, main).is_none() {
        return Veto {
            holds: Some(false),
            why: format!(
                "{head} is not the first {language} stream of the file, but stream {} ({}), which comes first, is lossy with two channels or fewer",
                first.stream, first.description
            ),
        };
    }
    Veto {
        holds: Some(true),
        why: format!(
            "{head} is not the first {language} stream of the file: stream {} ({}) comes first and is lossless or has more than two channels, so the reference choice may be inverted",
            first.stream, first.description
        ),
    }
}

/// "<candidate language> against <main language>" when both lines name a language (not NO_LANGUAGE) and they
/// differ, else "". Py: cross_language.
pub fn cross_language(main_info: &Fields, cand_info: &Fields) -> String {
    let a = cand_info.get("language").map(String::as_str).unwrap_or("");
    let b = main_info.get("language").map(String::as_str).unwrap_or("");
    if NO_LANGUAGE.iter().any(|code| *code == a) || NO_LANGUAGE.iter().any(|code| *code == b) || a == b {
        return String::new();
    }
    format!("{a} against {b}")
}

/// What `results` calls to measure a pair: the Python's analyse_fn(main, cand, dialogue). Plain is the Python's
/// default (analyse, no cache). Task 8's Measurer keeps the cache of spec 3.3 (keyed by sha1 of "lazymkv-rule:",
/// RULE_VERSION, the canonical constants JSON, then per series its little-endian f64 bytes and "|", or "|" alone for
/// none) and returns the key with the measures, in place of the Python's Measurer.key_of[id(m)] lookup. Err is
/// analyse's Err (the Python's ValueError from max(), which `results` passed on to its caller).
pub trait AnalyseFn {
    fn measure(&mut self, main: &[f64], cand: &[f64], dialogue: Option<&[f64]>) -> Result<Measured, String>;
}

/// Measures and the cache key they are kept under (None when no cache is used).
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub measures: Measures,
    pub key: Option<String>,
}

/// analyse(main, cand, dialogue, true, consts), no cache: the Python's default analyse_fn.
pub struct Plain<'c> {
    pub consts: &'c Constants,
}

impl AnalyseFn for Plain<'_> {
    fn measure(&mut self, main: &[f64], cand: &[f64], dialogue: Option<&[f64]>) -> Result<Measured, String> {
        Ok(Measured { measures: analyse(main, cand, dialogue, true, self.consts)?, key: None })
    }
}

/// Which curve the forward runs used as dialogue: the main stream's centre channel (main-fc.txt) or its mixdown.
/// Serialised "fc" | "mix".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dialogue {
    Fc,
    Mix,
}

/// A level folder's measures and verdicts as data. Py: results' dict.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderResults {
    /// tree.display(folder). Py: str(d).
    pub folder: String,
    pub manifest: Manifest,
    /// The manifest's main line (empty when none).
    pub main: Fields,
    /// Frames of main-mix.txt.
    pub frames: usize,
    /// Fc when main-fc.txt exists and is not all at or below SILENT_DB.
    pub dialogue: Dialogue,
    pub candidates: Vec<CandidateResult>,
    pub veto: Veto,
}

/// One candidate. Py: results' candidate dict.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateResult {
    /// The manifest's cand= value, a string as the Python keeps it.
    pub stream: String,
    pub info: Fields,
    /// The stream (main= or cand= value) with the same packet MD5, for an exact duplicate.
    pub duplicate_of: Option<String>,
    /// None for a duplicate.
    pub measures: Option<Measures>,
    /// The cache key of `measures` (Measured.key).
    pub key: Option<String>,
    pub verdict: Verdict,
    pub not_judged: String,
    pub cross_language: String,
    pub peer: Option<Peer>,
    /// What rows 3 to 9 said when the veto replaced it with ReferenceUnclear.
    pub before_veto: Option<Verdict>,
}

/// The peer run of a cross-language candidate. Py: results' "peer" dict.
#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub stream: String,
    pub measures: Measures,
    pub key: Option<String>,
    pub verdict: Verdict,
}

/// The measures and verdict of every candidate of a level folder written by audio-levels.ps1, as data, in the order
/// of audio_similarity.results(folder, rule, analyse_fn):
/// 1. refusal: Err with its reason before anything is read or measured;
/// 2. manifest.txt, main-mix.txt, and main-fc.txt when it exists and is not all at or below consts.SILENT_DB (then it
///    is the dialogue curve of the forward runs; otherwise they get none, and analyse uses the mixdown);
/// 3. the veto from streams.json for the manifest's main stream (holds None, why "not checked", without a main=
///    value);
/// 4. per candidate in manifest order: a duplicate of the main stream or of an earlier candidate by packet MD5 (no
///    measure), else the forward run against main-mix through `analyse_fn`, its verdict and not_judged;
/// 5. a cross-language candidate whose verdict is commentary gets commentary-cross-language, and when 8.6 places
///    another non-duplicate candidate of its language first, the peer run (that stream's levels as main, this one's
///    as candidate, no dialogue curve): note-commentary unless the peer verdict is commentary;
/// 6. when the veto holds, every candidate that is not a duplicate becomes reference-unclear, its earlier verdict
///    kept in before_veto.
///
/// Err also when a file cannot be read, a level does not parse, streams.json does not parse, a manifest stream
/// number is not an int, or `analyse_fn` fails (the Python's ValueError and OSError texts). `consts` is the set of measure constants in
/// force: SILENT_DB is read here, `analyse_fn` applies the others. Py: audio_similarity.results.
pub fn results(
    tree: &dyn Tree,
    folder: &PathKey,
    rule: &Rule,
    consts: &Constants,
    analyse_fn: &mut dyn AnalyseFn,
) -> Result<FolderResults, String> {
    let why = refusal(tree, folder)?;
    if !why.is_empty() {
        return Err(why);
    }
    let manifest: Manifest = read_manifest(tree, folder)?;
    let main = read_levels(tree, &folder.join("main-mix.txt"))?;
    let fc_path = folder.join("main-fc.txt");
    let mut fc = if tree.exists(&fc_path) { read_levels(tree, &fc_path)? } else { Vec::new() };
    if !fc.is_empty() && py_max(fc.iter().copied())? <= consts.SILENT_DB {
        fc = Vec::new();
    }
    let mainfo: Fields = manifest.main.clone().unwrap_or_default();
    // position_veto(read_streams(d), int(mainfo["main"])) if mainfo.get("main") else (None, "not checked"):
    // streams.json is read before the stream number is parsed, as the Python evaluates the arguments.
    let veto = match mainfo.get("main") {
        Some(stream) if !stream.is_empty() => {
            let streams = read_streams(tree, folder)?;
            position_veto(streams.as_deref(), int_text(Some(stream.as_str()))?)
        }
        _ => Veto { holds: None, why: "not checked".to_string() },
    };
    let dialogue: Option<&[f64]> = if fc.is_empty() { None } else { Some(fc.as_slice()) };
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    if let Some(md5) = mainfo.get("md5").filter(|md5| !md5.is_empty()) {
        seen.insert(md5.clone(), mainfo.get("main").cloned().unwrap_or_else(|| "?".to_string()));
    }
    let mut candidates: Vec<CandidateResult> = Vec::new();
    for c in &manifest.cands {
        // Every candidate line has cand= (parse_manifest files a line under cands by that key).
        let sid = c.get("cand").cloned().unwrap_or_default();
        let mut r = CandidateResult {
            stream: sid.clone(),
            info: c.clone(),
            duplicate_of: None,
            measures: None,
            key: None,
            verdict: Verdict::Duplicate,
            not_judged: String::new(),
            cross_language: cross_language(&mainfo, c),
            peer: None,
            before_veto: None,
        };
        let md5 = c.get("md5");
        match md5.and_then(|md5| seen.get(md5)).cloned() {
            Some(first) => r.duplicate_of = Some(first),
            None => {
                if let Some(md5) = md5.filter(|md5| !md5.is_empty()) {
                    seen.insert(md5.clone(), sid.clone());
                }
                let cand = read_levels(tree, &folder.join(&format!("cand-{sid}.txt")))?;
                let measured = analyse_fn.measure(&main, &cand, dialogue)?;
                r.verdict = verdict(&measured.measures, rule);
                r.not_judged = match measured.measures.error() {
                    Some(error) => error.to_string(),
                    None => not_judged(&measured.measures, rule),
                };
                r.measures = Some(measured.measures);
                r.key = measured.key;
            }
        }
        candidates.push(r);
    }
    for i in 0..candidates.len() {
        if candidates[i].verdict != Verdict::Commentary || candidates[i].cross_language.is_empty() {
            continue;
        }
        let mut own = Vec::new();
        for x in &candidates {
            if x.duplicate_of.is_none() && x.info.get("language") == candidates[i].info.get("language") {
                own.push(manifest_stream(&x.info)?);
            }
        }
        let peer = placed_first(&own).stream;
        candidates[i].verdict = Verdict::CommentaryCrossLanguage;
        if peer != int_text(Some(candidates[i].stream.as_str()))? {
            let peer_levels = read_levels(tree, &folder.join(&format!("cand-{peer}.txt")))?;
            let own_levels = read_levels(tree, &folder.join(&format!("cand-{}.txt", candidates[i].stream)))?;
            let measured = analyse_fn.measure(&peer_levels, &own_levels, None)?;
            let peer_verdict = verdict(&measured.measures, rule);
            let r = &mut candidates[i];
            r.peer = Some(Peer {
                stream: peer.to_string(),
                measures: measured.measures,
                key: measured.key,
                verdict: peer_verdict,
            });
            if peer_verdict != Verdict::Commentary {
                r.verdict = Verdict::NoteCommentary;
            }
        }
    }
    if veto.holds == Some(true) {
        for r in candidates.iter_mut().filter(|r| r.duplicate_of.is_none()) {
            r.before_veto = Some(r.verdict);
            r.verdict = Verdict::ReferenceUnclear;
        }
    }
    Ok(FolderResults {
        folder: tree.display(folder),
        frames: main.len(),
        dialogue: if fc.is_empty() { Dialogue::Mix } else { Dialogue::Fc },
        main: mainfo,
        manifest,
        candidates,
        veto,
    })
}

#[cfg(test)]
mod tests;
