//! The owner's labels for the subtitle tracks of one disc, `<slug>/subtitle-labels.json` (RP15; design spec 15
//! "Recordings" and its `retune` paragraph, section 17 row 14, D20, D23): what each English or no-language subtitle
//! track of a rip label is, kept beside `audio-labels.json` and in its shape. The file is a `serde_json::Value` with
//! insertion order kept, so an unknown key and every character of a note survive an edit; `problems` validates the
//! value in the wording of `audio::labels::problems`, and the typed `SubtitleLabel`s are made only after it passes.
//! The edits (`set_track`, `unset_track`) are functions from the value to the new value that refuse a change adding
//! a problem; `lazymkv-dev labels --subtitles` reads and writes the file. Which labels count towards row 14 is Task
//! 18's (`retune --subtitles`): only a `read` or `watched` label given before the verdict (`saw_verdict` false) can
//! (D23, section 17 row 14 (a)), and a counted `sdh` label with `style` "uk" is row 14's UK-style SDH track (RP25).
//! The validation this file shares
//! with the audio labels (the head checks, the `removed` list, an entry's keys, label, stream and date, and its
//! history) and the helpers it needs (the value accessors, the JSON reader, the `how` names and the sort key) are
//! `audio::labels`' own, called rather than copied (RP15). Written from the spec (RP20); no I/O (RP19): files come through a
//! `Tree`. Used by Tasks 17 and 18.

use std::str::FromStr;

use serde_json::{json, Map, Value};

use crate::audio::labels::{
    base_entry_problems, get, head_problems, history_problems, names, read_value, removed_problems, repr_of, track_sort_key,
};
use crate::recording::{py_repr_str, py_str, PathKey, Tree};

/// The subtitle labels file's format line, as `audio::labels::FORMAT` is the audio labels'. Spec 15, RP15.
pub const FORMAT: &str = "lazymkv subtitle labels v1";
/// The subtitle labels file's name in a recording folder, beside `audio-labels.json`; the one copy of the name, which
/// `lazymkv-dev labels --subtitles` and Task 18's harness both use. Spec 15, RP15.
pub const FILE_NAME: &str = "subtitle-labels.json";

/// (kind, what the owner wants in the final file, meaning), in RP15's order. What the owner wants follows R5, R6 and
/// D20: the dialogue kinds (regular, SDH, translation, dub transcript, closed captions) and a forced track are kept;
/// commentary is never kept (R6); trivia and lyrics are dropped when confirmed (D20); "ask" for a track no kind
/// describes. Spec 15, section 17 row 14, RP15.
pub const KINDS: [(&str, &str, &str); 10] = [
    ("regular", "keep", "plain dialogue subtitles: the words spoken, with no sound cues or speaker labels (R5, D20)"),
    ("sdh", "keep", "subtitles for the deaf and hard of hearing: the dialogue with sound cues and speaker labels (R5, D20)"),
    ("forced", "keep", "forced subtitles: only the lines for speech in another language, signs and captions (R5, 8.7)"),
    ("commentary", "drop", "a subtitled commentary (R6)"),
    ("trivia", "drop", "trivia or pop-up text (D20: dropped when confirmed)"),
    ("lyrics", "drop", "lyrics or a sing-along track (D20: dropped when confirmed)"),
    ("translation", "keep", "an English translation of the dialogue of a film in another language (R4, R5)"),
    ("dub-transcript", "keep", "a transcript of the English dub, kept beside the translation (D20)"),
    ("closed-captions", "keep", "the DVD's closed captions, which MakeMKV converts to text; SDH by type (8.7)"),
    ("other", "ask", "a track none of the kinds above describes; the note says what it is"),
];

/// (how, meaning): where a label comes from, in RP15's order with ruling RP25's `flag` after the menu, as row 14 lists
/// the observations. Row 14 and D23: a track counts only when its text was read, by OCR text or by watching it; the
/// packaging, the menu, a flag, the player and TheDiscDB are observations. Spec 17 row 14, D23, RP15, RP25.
pub const HOW: [(&str, &str); 7] = [
    ("read", "a person read its OCR text (counts, D23)"),
    ("watched", "a person watched it on screen with the programme (counts, D23)"),
    ("packaging", "the packaging says so (an observation: does not count on its own, D23)"),
    ("menu", "the disc's menu says so (an observation: does not count on its own, D23)"),
    ("flag", "a flag in MakeMKV's scan (an observation: does not count on its own, D23)"),
    ("player", "a player's track list says so (an observation: does not count on its own, D23)"),
    ("thediscdb", "TheDiscDB's track description (an observation: does not count on its own, D23)"),
];
/// D23's counting sources: the two ways a person checks a subtitle track's text. Spec 17 row 14, D23.
pub const COUNTING_HOW: [&str; 2] = ["read", "watched"];
/// Sources kept with a label as observations, never counted on their own. Spec 17 row 14, D23.
pub const OBSERVATION_HOW: [&str; 5] = ["packaging", "menu", "flag", "player", "thediscdb"];

/// RP15's kinds of subtitle track, as the owner labels them; serialised as their spellings in `KINDS`. Spec 15,
/// section 17 row 14, RP15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubtitleKind {
    /// Plain dialogue subtitles.
    Regular,
    /// Subtitles for the deaf and hard of hearing.
    Sdh,
    /// Forced subtitles.
    Forced,
    /// A subtitled commentary.
    Commentary,
    /// Trivia or pop-up text.
    Trivia,
    /// Lyrics or a sing-along track.
    Lyrics,
    /// An English translation of a film in another language.
    Translation,
    /// A transcript of the English dub.
    DubTranscript,
    /// The DVD's closed captions.
    ClosedCaptions,
    /// Anything else.
    Other,
}

impl SubtitleKind {
    /// Every kind in `KINDS` order. Spec 15, section 17 row 14, RP15.
    pub const ALL: [SubtitleKind; 10] = [
        SubtitleKind::Regular,
        SubtitleKind::Sdh,
        SubtitleKind::Forced,
        SubtitleKind::Commentary,
        SubtitleKind::Trivia,
        SubtitleKind::Lyrics,
        SubtitleKind::Translation,
        SubtitleKind::DubTranscript,
        SubtitleKind::ClosedCaptions,
        SubtitleKind::Other,
    ];

    /// The kind's spelling in the file and in `KINDS`. Spec 15, section 17 row 14, RP15.
    pub fn as_str(self) -> &'static str {
        match self {
            SubtitleKind::Regular => "regular",
            SubtitleKind::Sdh => "sdh",
            SubtitleKind::Forced => "forced",
            SubtitleKind::Commentary => "commentary",
            SubtitleKind::Trivia => "trivia",
            SubtitleKind::Lyrics => "lyrics",
            SubtitleKind::Translation => "translation",
            SubtitleKind::DubTranscript => "dub-transcript",
            SubtitleKind::ClosedCaptions => "closed-captions",
            SubtitleKind::Other => "other",
        }
    }
}

impl FromStr for SubtitleKind {
    type Err = String;

    /// The kind a label spells, exactly; Err "'<text>' is not one of <the ten spellings, joined by ', '>" for anything
    /// else. Spec 15, RP15.
    fn from_str(text: &str) -> Result<SubtitleKind, String> {
        SubtitleKind::ALL.into_iter().find(|kind| kind.as_str() == text).ok_or_else(|| {
            let names: Vec<&str> = SubtitleKind::ALL.iter().map(|kind| kind.as_str()).collect();
            format!("{} is not one of {}", py_repr_str(text), names.join(", "))
        })
    }
}

/// One entry of `subtitle-labels.json`, read from a file that validates; the keys in the file's order. Spec 15, RP15, RP25.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubtitleLabel {
    /// The rip label (the folder of the rip under the recording).
    pub label: String,
    /// ffprobe's stream number of the track in the rip label's file (mkvmerge's track id).
    pub stream: i64,
    /// What the track is.
    pub kind: SubtitleKind,
    /// Where the label comes from: one of `HOW`.
    pub how: String,
    /// Whether the owner is sure.
    pub sure: bool,
    /// When the label was given, YYYY-MM-DD.
    pub date: String,
    /// Whether the owner saw the rules' verdict on the track before giving the label; such a label never counts
    /// (section 17 row 14 (a): "checked before the verdict", D23), as in `audio-labels.json`.
    pub saw_verdict: bool,
    /// The owner's note; null in the file when there is none.
    pub note: Option<String>,
    /// "uk" for a UK-style SDH track (speakers by colour, sounds without brackets), which section 17 row 14 asks for if
    /// the owner has a UK disc; null in the file otherwise. RP25.
    pub style: Option<String>,
}

/// Every key a track entry has: RP15's fields and ruling RP25's two, `saw_verdict` (as in `audio-labels.json`, section 17
/// row 14 (a)) and `style` (row 14's UK-style SDH track). Spec 15, RP15, RP25.
const TRACK_KEYS: [&str; 9] = ["label", "stream", "kind", "how", "sure", "date", "saw_verdict", "note", "style"];
/// The other key a track may have: the labels it replaced.
const OPTIONAL_KEYS: [&str; 1] = ["history"];
/// The other keys a history or removed entry may have (checked only for what reading it needs).
const LOOSE_KEYS: [&str; 4] = ["history", "replaced", "removed", "why"];
/// The file's top-level keys: `audio-labels.json`'s without `titles`.
const TOP_KEYS: [&str; 4] = ["format", "slug", "tracks", "removed"];

/// Every problem of a parsed labels file, one sentence each, in `audio::labels::problems`' wording with the
/// subtitle vocabulary; empty when it is valid. `slug` is the name of the folder the file is in (None: not compared).
/// The checks, in order: the top-level keys, the format line, the slug, the `removed` list (each entry read loosely,
/// with its `removed` date), the `tracks` list, then each track's keys, label, stream, date, kind, how,
/// saw_verdict and sure, note, style and history, and a (label, stream) labelled twice. The head, the `removed` list,
/// each entry's keys, label, stream and date, and its history are checked by the helpers `audio::labels` shares with
/// this file (`head_problems`, `removed_problems`, `base_entry_problems`, `history_problems`; RP15), so the two files
/// say those sentences through one copy of the code. Spec 15, section 17 row 14, D23, RP15, RP25.
pub fn problems(data: &Value, slug: Option<&str>) -> Vec<String> {
    let Some(d) = data.as_object() else { return vec!["the file is not a JSON object".to_string()] };
    let mut out = head_problems(d, &TOP_KEYS, FORMAT, slug);
    out.extend(removed_problems(d, |r, where_| entry_problems(r, where_, false)));
    let Some(tracks) = get(d, "tracks").as_array() else {
        out.push("tracks is not a list".to_string());
        return out;
    };
    let mut seen: Vec<(String, i64)> = Vec::new();
    for (k, t) in tracks.iter().enumerate() {
        let Some(tm) = t.as_object() else {
            out.push(format!("track {k}: not an object"));
            continue;
        };
        let where_ = format!("track {} stream {}", py_str(get(tm, "label")), py_str(get(tm, "stream")));
        out.extend(entry_problems(t, &where_, true));
        if let Some(key) = key_of(tm) {
            if seen.contains(&key) {
                out.push(format!("{where_}: labelled twice"));
            }
            seen.push(key);
        }
    }
    out
}

/// The problems of one entry, each prefixed with `where_`. `strict` false (history and removed entries) checks only
/// that the entry can be read: its keys, label, stream and date (`audio::labels::base_entry_problems` with this file's
/// keys), so its kind and how may come from an older vocabulary. A strict entry adds this file's kind, how,
/// saw_verdict and sure, note and style checks, then its history (`audio::labels::history_problems`). A value that is
/// not an object is read as an empty one. Spec 15, RP15.
fn entry_problems(t: &Value, where_: &str, strict: bool) -> Vec<String> {
    let empty = Map::new();
    let m = t.as_object().unwrap_or(&empty);
    let allowed: &[&str] = if strict { &OPTIONAL_KEYS } else { &LOOSE_KEYS };
    let mut out = base_entry_problems(m, where_, &TRACK_KEYS, allowed);
    if !strict {
        return out;
    }
    if !get(m, "kind").as_str().map_or(false, |k| SubtitleKind::from_str(k).is_ok()) {
        out.push(format!("{where_}: kind {} is not in the vocabulary (lazymkv-dev labels --subtitles kinds)", repr_of(m, "kind")));
    }
    if !get(m, "how").as_str().map_or(false, |h| HOW.iter().any(|(name, _)| *name == h)) {
        out.push(format!("{where_}: how {} is not one of {}", repr_of(m, "how"), names(&HOW).join(", ")));
    }
    for key in ["saw_verdict", "sure"] {
        if !get(m, key).is_boolean() {
            out.push(format!("{where_}: {key} must be true or false"));
        }
    }
    let note = get(m, "note");
    if !note.is_string() && !note.is_null() {
        out.push(format!("{where_}: note is not text"));
    }
    let style = get(m, "style");
    if !style.is_null() && style.as_str() != Some("uk") {
        out.push(format!("{where_}: style {} is not null or 'uk'", repr_of(m, "style")));
    }
    out.extend(history_problems(m, where_, |h, w| entry_problems(h, w, false)));
    out
}

/// An entry's (label, stream) when the label is text and the stream an integer; None otherwise (such an entry has
/// its own problems already).
fn key_of(m: &Map<String, Value>) -> Option<(String, i64)> {
    Some((m.get("label")?.as_str()?.to_string(), m.get("stream")?.as_i64()?))
}

/// Reads and validates the labels file at `path`; the slug is compared with the resolved name of the folder the file
/// is in (`Tree::resolved_name` of its parent), so a file read through a tree rooted at its own folder is compared
/// with that folder's real name. Err "<shown>: not valid JSON (<the reason>)", "<shown>: <problems joined by '; '>", or the
/// read error's text. Spec 15, RP15.
pub fn load(tree: &dyn Tree, path: &PathKey) -> Result<Vec<SubtitleLabel>, String> {
    let shown = tree.display(path);
    let data = read_value(tree, path)?.map_err(|e| format!("{shown}: not valid JSON ({e})"))?;
    let found = problems(&data, Some(&tree.resolved_name(&path.parent())));
    if !found.is_empty() {
        return Err(format!("{shown}: {}", found.join("; ")));
    }
    data["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| serde_json::from_value::<SubtitleLabel>(t.clone()).map_err(|e| format!("{shown}: {e}")))
        .collect()
}

/// What `set_track` compares to decide that a label is unchanged: the date says when, not what. Spec 15, RP15.
const SUBSTANCE: [&str; 6] = ["kind", "how", "sure", "saw_verdict", "note", "style"];
/// An edit's refusal of a value that has no tracks list.
const NO_TRACKS: &str = "the file has no 'tracks' list, so nothing is changed";

/// Adds or replaces the label of (`label.label`, `label.stream`) in the file's value. One old label of the track with
/// the same kind, how, sure, saw_verdict, note and style is kept as it is, and the value is returned unchanged (not sorted, so the caller
/// sees that there is nothing to write); otherwise every old label of the track goes into the new entry's `history`
/// (each one's own history first) with `replaced` set to the new date, and the tracks are sorted stably by (label,
/// stream). Returns the new value and the problems it still has; Err "not written: <the problems the change adds>"
/// when the change adds a sentence the value did not have, and Err when the value has no tracks list. The slug is not
/// compared (an edit never changes it; `load` compares it). Spec 15, RP15, D16 ("the owner's label for a track, once
/// given, is kept with it").
pub fn set_track(data: Value, label: &SubtitleLabel) -> Result<(Value, Vec<String>), String> {
    let tracks: Vec<Value> = tracks_of(&data)?.clone();
    let before = problems(&data, None);
    let mut entry = serde_json::to_value(label).map_err(|e| e.to_string())?;
    let key = Some((label.label.clone(), label.stream));
    let is_old = |t: &Value| t.as_object().and_then(key_of) == key;
    let old: Vec<&Value> = tracks.iter().filter(|t| is_old(*t)).collect();
    if old.len() == 1 && SUBSTANCE.iter().all(|k| old[0].get(*k) == entry.get(*k)) {
        return Ok((data, before));
    }
    if !old.is_empty() {
        let mut history = Vec::new();
        for o in &old {
            if let Some(Value::Array(earlier)) = o.get("history") {
                history.extend(earlier.iter().cloned());
            }
            let mut copy: Map<String, Value> = o
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(k, _)| k.as_str() != "history")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            copy.insert("replaced".to_string(), json!(label.date));
            history.push(Value::Object(copy));
        }
        entry["history"] = Value::Array(history);
    }
    let mut kept: Vec<Value> = tracks.iter().filter(|t| !is_old(*t)).cloned().collect();
    kept.push(entry);
    kept.sort_by(|a, b| track_sort_key(a).cmp(&track_sort_key(b)));
    let mut new = data;
    new["tracks"] = Value::Array(kept);
    checked(&before, new)
}

/// Moves every label of (`label`, `stream`) to the file's `removed` list (added at the end of the file when absent),
/// each with `removed` (the date) and `why`, and sorts the tracks left as `set_track` does. Returns the new value and
/// the problems it still has; Err "<label> stream <n> has no label", "unset needs --note: why the label is taken
/// away", "'removed' is not a list, so the label cannot be moved there", "not written: <the new problems>" as `set_track`, or the
/// tracks-list refusal. As `audio::labels::unset_track` does. Spec 15, RP15, D16.
pub fn unset_track(data: Value, label: &str, stream: i64, why: &str, date: &str) -> Result<(Value, Vec<String>), String> {
    let tracks: Vec<Value> = tracks_of(&data)?.clone();
    let key = Some((label.to_string(), stream));
    let is_old = |t: &Value| t.as_object().and_then(key_of) == key;
    let old: Vec<Value> = tracks.iter().filter(|t| is_old(*t)).cloned().collect();
    if old.is_empty() {
        return Err(format!("{label} stream {stream} has no label"));
    }
    if why.is_empty() {
        return Err("unset needs --note: why the label is taken away".to_string());
    }
    let before = problems(&data, None);
    let mut kept: Vec<Value> = tracks.iter().filter(|t| !is_old(*t)).cloned().collect();
    kept.sort_by(|a, b| track_sort_key(a).cmp(&track_sort_key(b)));
    let moved: Vec<Value> = old
        .into_iter()
        .map(|o| {
            let mut m = o.as_object().cloned().unwrap_or_default();
            m.insert("removed".to_string(), json!(date));
            m.insert("why".to_string(), json!(why));
            Value::Object(m)
        })
        .collect();
    let mut new = data;
    new["tracks"] = Value::Array(kept);
    let d = new.as_object_mut().expect("a value with a tracks list is an object");
    match d.get_mut("removed") {
        None => {
            d.insert("removed".to_string(), Value::Array(moved));
        }
        Some(Value::Array(list)) => list.extend(moved),
        Some(_) => return Err("'removed' is not a list, so the label cannot be moved there".to_string()),
    }
    checked(&before, new)
}

/// The value's tracks list, or the refusal of an edit.
fn tracks_of(data: &Value) -> Result<&Vec<Value>, String> {
    data.get("tracks").and_then(Value::as_array).ok_or_else(|| NO_TRACKS.to_string())
}

/// The edited value and its problems, or Err "not written: <the problems `before` did not have, joined by '; '>".
fn checked(before: &[String], new: Value) -> Result<(Value, Vec<String>), String> {
    let after = problems(&new, None);
    let added: Vec<&str> = after.iter().filter(|p| !before.contains(*p)).map(String::as_str).collect();
    if !added.is_empty() {
        return Err(format!("not written: {}", added.join("; ")));
    }
    Ok((new, after))
}

// ------------------------------------------------------------------ row 14 and the harness (Task 18)

pub use row14::{costly, exp14_requirements, harness, Costly, Outcome, Row14, SubtitleHarness};

/// Spec section 17 row 14 (a) read from the spec text, its costly verdicts, and the subtitle harness behind
/// `lazymkv-dev retune --subtitles` (RP15; spec 15 "Rules against the stored evidence"; 8.7 "Text"; Task 18). Private:
/// what other code uses is re-exported above it.
mod row14;

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::{json, Value};

    use crate::recording::{MemTree, PathKey};
    use crate::testsupport::with;

    /// A subtitle label as `labels --subtitles set` writes it: how "read", sure, dated 2026-10-14, given before the
    /// verdict, no note, no style.
    fn label(name: &str, stream: i64, kind: &str) -> Value {
        json!({
            "label": name, "stream": stream, "kind": kind, "how": "read", "sure": true, "date": "2026-10-14",
            "saw_verdict": false, "note": null, "style": null,
        })
    }

    /// The typed form of `label(name, stream, kind.as_str())`.
    fn typed(name: &str, stream: i64, kind: SubtitleKind) -> SubtitleLabel {
        SubtitleLabel {
            label: name.to_string(),
            stream,
            kind,
            how: "read".to_string(),
            sure: true,
            date: "2026-10-14".to_string(),
            saw_verdict: false,
            note: None,
            style: None,
        }
    }

    /// A subtitle labels file of the recording disc-b holding `tracks`.
    fn file(tracks: Vec<Value>) -> Value {
        json!({"format": FORMAT, "slug": "disc-b", "tracks": tracks})
    }

    /// `value` (an object) without `key`.
    fn without(mut value: Value, key: &str) -> Value {
        value.as_object_mut().expect("an object").remove(key);
        value
    }

    /// A tree printed under corpus/owner holding disc-b/subtitle-labels.json with `bytes`.
    fn tree_with(bytes: &[u8]) -> MemTree {
        let mut tree = MemTree::new("corpus/owner", "owner");
        tree.insert_file("disc-b/subtitle-labels.json", bytes.to_vec());
        tree
    }

    #[test]
    fn the_kinds_are_rp15s_ten_in_order_with_what_the_owner_wants() {
        let names: Vec<&str> = KINDS.iter().map(|(kind, _, _)| *kind).collect();
        let spelled: Vec<&str> = SubtitleKind::ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(names, spelled);
        assert_eq!(
            names,
            ["regular", "sdh", "forced", "commentary", "trivia", "lyrics", "translation", "dub-transcript", "closed-captions", "other"]
        );
        // D20 and R5: the dialogue kinds and the forced track are kept; R6 and D20: commentary, trivia and lyrics are
        // dropped when confirmed; a track no kind describes is the owner's question.
        let wanted: Vec<&str> = KINDS.iter().map(|(_, wanted, _)| *wanted).collect();
        assert_eq!(wanted, ["keep", "keep", "keep", "drop", "drop", "drop", "keep", "keep", "keep", "ask"]);
        assert!(KINDS.iter().all(|(_, _, meaning)| !meaning.is_empty()));
    }

    #[test]
    fn the_sources_are_d23s_two_checks_of_the_text_and_five_observations() {
        let names: Vec<&str> = HOW.iter().map(|(how, _)| *how).collect();
        assert_eq!(names, ["read", "watched", "packaging", "menu", "flag", "player", "thediscdb"]);
        assert_eq!(HOW[4], ("flag", "a flag in MakeMKV's scan (an observation: does not count on its own, D23)"));
        let sources: Vec<&str> = COUNTING_HOW.iter().chain(OBSERVATION_HOW.iter()).copied().collect();
        assert_eq!(names, sources);
        for (how, meaning) in HOW {
            if COUNTING_HOW.contains(&how) {
                assert!(meaning.ends_with("(counts, D23)"), "{how}: {meaning}");
            } else {
                assert!(meaning.ends_with("(an observation: does not count on its own, D23)"), "{how}: {meaning}");
            }
        }
    }

    #[test]
    fn a_kind_is_spelled_the_same_by_as_str_serde_and_from_str() {
        for kind in SubtitleKind::ALL {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(kind.as_str()));
            assert_eq!(serde_json::from_value::<SubtitleKind>(json!(kind.as_str())).unwrap(), kind);
            assert_eq!(SubtitleKind::from_str(kind.as_str()), Ok(kind));
        }
        assert_eq!(SubtitleKind::DubTranscript.as_str(), "dub-transcript");
        assert_eq!(SubtitleKind::ClosedCaptions.as_str(), "closed-captions");
        let all = "regular, sdh, forced, commentary, trivia, lyrics, translation, dub-transcript, closed-captions, other";
        assert_eq!(SubtitleKind::from_str("SDH"), Err(format!("'SDH' is not one of {all}")));
        assert_eq!(SubtitleKind::from_str("closed_captions"), Err(format!("'closed_captions' is not one of {all}")));
        assert!(serde_json::from_value::<SubtitleKind>(json!("closed_captions")).is_err());
    }

    #[test]
    fn a_label_serialises_with_the_files_keys_in_order() {
        let mut dub = typed("all-tracks", 4, SubtitleKind::DubTranscript);
        assert_eq!(
            serde_json::to_string(&dub).unwrap(),
            r#"{"label":"all-tracks","stream":4,"kind":"dub-transcript","how":"read","sure":true,"date":"2026-10-14","saw_verdict":false,"note":null,"style":null}"#
        );
        assert_eq!(serde_json::to_value(&dub).unwrap(), label("all-tracks", 4, "dub-transcript"));
        dub.note = Some("the English dub's words".to_string());
        let text = serde_json::to_string(&dub).unwrap();
        assert!(text.ends_with(r#""note":"the English dub's words","style":null}"#), "{text}");
        assert_eq!(serde_json::from_str::<SubtitleLabel>(&text).unwrap(), dub);
    }

    /// The problems of disc-b's file holding the one track `entry`.
    fn track_problems(entry: Value) -> Vec<String> {
        problems(&file(vec![entry]), Some("disc-b"))
    }

    #[test]
    fn a_valid_file_has_no_problems() {
        let data = json!({
            "format": FORMAT,
            "slug": "disc-b",
            "tracks": [
                label("all-tracks", 4, "sdh"),
                with(
                    label("all-tracks", 5, "regular"),
                    "history",
                    json!([with(label("all-tracks", 5, "sdh"), "replaced", json!("2026-10-15"))]),
                ),
                with(label("eng-only", 3, "closed-captions"), "note", json!("CC->Text English ( Lossy conversion )")),
            ],
            "removed": [with(with(label("all-tracks", 6, "forced"), "removed", json!("2026-10-16")), "why", json!("wrong stream"))],
        });
        assert_eq!(problems(&data, Some("disc-b")), Vec::<String>::new());
        assert_eq!(problems(&data, None), Vec::<String>::new());
        assert_eq!(problems(&file(vec![]), Some("disc-b")), Vec::<String>::new());
    }

    #[test]
    fn a_value_that_is_not_an_object_is_one_problem() {
        for data in [json!([]), json!("labels"), json!(null)] {
            assert_eq!(problems(&data, Some("disc-b")), ["the file is not a JSON object"]);
        }
    }

    #[test]
    fn an_unknown_top_level_key_is_a_problem() {
        // The audio file's `titles` object is not part of the subtitle file (RP15 names none).
        let data = with(with(file(vec![]), "titles", json!({})), "extra", json!(1));
        assert_eq!(problems(&data, Some("disc-b")), ["unknown top-level keys ['extra', 'titles']"]);
    }

    #[test]
    fn the_format_line_must_be_the_subtitle_labels_one() {
        let audio = with(file(vec![]), "format", json!("lazymkv audio labels v1"));
        assert_eq!(problems(&audio, Some("disc-b")), ["format is 'lazymkv audio labels v1', not 'lazymkv subtitle labels v1'"]);
        assert_eq!(problems(&without(file(vec![]), "format"), Some("disc-b")), ["format is None, not 'lazymkv subtitle labels v1'"]);
    }

    #[test]
    fn the_slug_must_be_a_slug_and_name_the_folder_the_file_is_in() {
        assert_eq!(
            problems(&with(file(vec![]), "slug", json!("Disc B")), None),
            ["slug 'Disc B' is not lowercase letters, digits and hyphens"]
        );
        assert_eq!(problems(&with(file(vec![]), "slug", json!(7)), None), ["slug 7 is not lowercase letters, digits and hyphens"]);
        assert_eq!(problems(&file(vec![]), Some("disc-c")), ["slug is 'disc-b' but the file is in corpus/owner/disc-c"]);
        assert_eq!(problems(&file(vec![]), None), Vec::<String>::new());
    }

    #[test]
    fn tracks_must_be_a_list_of_objects() {
        assert_eq!(problems(&with(file(vec![]), "tracks", json!({})), None), ["tracks is not a list"]);
        assert_eq!(problems(&without(file(vec![]), "tracks"), None), ["tracks is not a list"]);
        assert_eq!(problems(&file(vec![json!("all-tracks 4")]), None), ["track 0: not an object"]);
    }

    #[test]
    fn a_missing_or_unknown_key_of_a_track_is_a_problem() {
        // film_audible is an audio label's key, not a subtitle label's; a missing saw_verdict is also not true or false.
        let entry = without(without(without(label("all-tracks", 4, "sdh"), "date"), "note"), "saw_verdict");
        assert_eq!(
            track_problems(with(entry, "film_audible", json!(true))),
            [
                "track all-tracks stream 4: missing ['date', 'note', 'saw_verdict']",
                "track all-tracks stream 4: unknown keys ['film_audible']",
                "track all-tracks stream 4: date None is not YYYY-MM-DD",
                "track all-tracks stream 4: saw_verdict must be true or false",
            ]
        );
    }

    #[test]
    fn the_label_must_be_a_rip_label() {
        assert_eq!(track_problems(label("All-Tracks", 4, "sdh")), ["track All-Tracks stream 4: label is not a rip label"]);
        assert_eq!(track_problems(with(label("x", 4, "sdh"), "label", json!(7))), ["track 7 stream 4: label is not a rip label"]);
    }

    #[test]
    fn the_stream_must_be_ffprobes_stream_number() {
        for (stream, shown) in [(json!(-1), "-1"), (json!("4"), "4"), (json!(4.0), "4.0"), (json!(true), "True")] {
            assert_eq!(
                track_problems(with(label("all-tracks", 0, "sdh"), "stream", stream)),
                [format!("track all-tracks stream {shown}: stream is not ffprobe's stream number (an integer, 0 or more)")]
            );
        }
        assert_eq!(track_problems(label("all-tracks", 0, "sdh")), Vec::<String>::new());
    }

    #[test]
    fn the_date_must_be_a_real_yyyy_mm_dd_date() {
        for date in ["2026-02-30", "16/10/2026", "2026-10-1"] {
            assert_eq!(
                track_problems(with(label("all-tracks", 4, "sdh"), "date", json!(date))),
                [format!("track all-tracks stream 4: date '{date}' is not YYYY-MM-DD")]
            );
        }
    }

    #[test]
    fn the_kind_must_be_one_of_the_ten() {
        for (kind, shown) in [(json!("dub"), "'dub'"), (json!("SDH"), "'SDH'"), (json!(null), "None")] {
            assert_eq!(
                track_problems(with(label("all-tracks", 4, "sdh"), "kind", kind)),
                [format!("track all-tracks stream 4: kind {shown} is not in the vocabulary (lazymkv-dev labels --subtitles kinds)")]
            );
        }
    }

    #[test]
    fn the_how_must_be_one_of_the_seven_sources() {
        for how in ["listened", "Flag", "md5"] {
            assert_eq!(
                track_problems(with(label("all-tracks", 4, "sdh"), "how", json!(how))),
                [format!("track all-tracks stream 4: how '{how}' is not one of read, watched, packaging, menu, flag, player, thediscdb")]
            );
        }
        for how in ["watched", "packaging", "menu", "flag", "player", "thediscdb"] {
            assert_eq!(track_problems(with(label("all-tracks", 4, "sdh"), "how", json!(how))), Vec::<String>::new());
        }
    }

    #[test]
    fn saw_verdict_and_sure_must_be_true_or_false() {
        for value in [json!("yes"), json!(1), json!(null)] {
            for key in ["saw_verdict", "sure"] {
                assert_eq!(
                    track_problems(with(label("all-tracks", 4, "sdh"), key, value.clone())),
                    [format!("track all-tracks stream 4: {key} must be true or false")]
                );
            }
        }
        let both = with(with(label("all-tracks", 4, "sdh"), "sure", json!("no")), "saw_verdict", json!("no"));
        assert_eq!(
            track_problems(both),
            ["track all-tracks stream 4: saw_verdict must be true or false", "track all-tracks stream 4: sure must be true or false"]
        );
        assert_eq!(track_problems(with(label("all-tracks", 4, "sdh"), "sure", json!(false))), Vec::<String>::new());
        assert_eq!(track_problems(with(label("all-tracks", 4, "sdh"), "saw_verdict", json!(true))), Vec::<String>::new());
    }

    #[test]
    fn the_note_is_text_or_null() {
        for note in [json!(3), json!(["cues"]), json!({})] {
            assert_eq!(track_problems(with(label("all-tracks", 4, "sdh"), "note", note)), ["track all-tracks stream 4: note is not text"]);
        }
        for note in [json!(null), json!(""), json!("cues in brackets")] {
            assert_eq!(track_problems(with(label("all-tracks", 4, "sdh"), "note", note)), Vec::<String>::new());
        }
    }

    #[test]
    fn the_style_is_null_or_uk() {
        for (style, shown) in [(json!("UK"), "'UK'"), (json!("british"), "'british'"), (json!(1), "1"), (json!(true), "True")] {
            assert_eq!(
                track_problems(with(label("all-tracks", 4, "sdh"), "style", style)),
                [format!("track all-tracks stream 4: style {shown} is not null or 'uk'")]
            );
        }
        for style in [json!(null), json!("uk")] {
            assert_eq!(track_problems(with(label("all-tracks", 4, "sdh"), "style", style)), Vec::<String>::new());
        }
        // A UK style on another kind is valid in the file; Task 18 counts it only on an sdh label.
        assert_eq!(track_problems(with(label("all-tracks", 4, "regular"), "style", json!("uk"))), Vec::<String>::new());
    }

    #[test]
    fn a_track_labelled_twice_is_a_problem() {
        let twice = file(vec![label("all-tracks", 4, "sdh"), label("all-tracks", 5, "sdh"), label("all-tracks", 4, "regular")]);
        assert_eq!(problems(&twice, None), ["track all-tracks stream 4: labelled twice"]);
        let other_label = file(vec![label("all-tracks", 4, "sdh"), label("eng-only", 4, "sdh")]);
        assert_eq!(problems(&other_label, None), Vec::<String>::new());
    }

    #[test]
    fn a_history_entry_is_checked_only_for_what_reading_it_needs_and_its_replaced_date() {
        // An older vocabulary survives in a history entry: its kind and how are not checked.
        let old_vocabulary = with(with(label("all-tracks", 4, "subtitles"), "how", json!("listened")), "replaced", json!("2026-10-15"));
        assert_eq!(track_problems(with(label("all-tracks", 4, "sdh"), "history", json!([old_vocabulary]))), Vec::<String>::new());
        assert_eq!(
            track_problems(with(label("all-tracks", 4, "sdh"), "history", json!({}))),
            ["track all-tracks stream 4: history is not a list"]
        );
        assert_eq!(
            track_problems(with(label("all-tracks", 4, "sdh"), "history", json!(["sdh"]))),
            ["track all-tracks stream 4: history 0 is not an object"]
        );
        let undated = without(label("all-tracks", 4, "regular"), "date");
        assert_eq!(
            track_problems(with(label("all-tracks", 4, "sdh"), "history", json!([undated]))),
            [
                "track all-tracks stream 4 history 0: missing ['date']",
                "track all-tracks stream 4 history 0: date None is not YYYY-MM-DD",
                "track all-tracks stream 4 history 0: replaced None is not YYYY-MM-DD",
            ]
        );
    }

    #[test]
    fn a_removed_entry_is_checked_only_for_what_reading_it_needs_and_its_removed_date() {
        let moved = with(with(label("all-tracks", 6, "forced"), "removed", json!("2026-10-16")), "why", json!("wrong stream"));
        assert_eq!(problems(&with(file(vec![]), "removed", json!([moved])), None), Vec::<String>::new());
        assert_eq!(problems(&with(file(vec![]), "removed", json!({})), None), ["removed is not a list"]);
        assert_eq!(problems(&with(file(vec![]), "removed", json!([7])), None), ["removed 0: not an object"]);
        let undated = with(label("all-tracks", 6, "forced"), "why", json!("wrong stream"));
        assert_eq!(problems(&with(file(vec![]), "removed", json!([undated])), None), ["removed 0: removed None is not YYYY-MM-DD"]);
        let audio_key = with(with(label("all-tracks", 6, "forced"), "removed", json!("2026-10-16")), "film_audible", json!(true));
        assert_eq!(problems(&with(file(vec![]), "removed", json!([audio_key])), None), ["removed 0: unknown keys ['film_audible']"]);
    }

    #[test]
    fn load_gives_the_typed_labels_in_file_order() {
        let replaced = json!([with(label("all-tracks", 4, "regular"), "replaced", json!("2026-10-15"))]);
        let data = file(vec![
            label("t1", 2, "regular"),
            with(with(label("all-tracks", 4, "sdh"), "note", json!("cues")), "history", replaced),
        ]);
        let mut bytes = "\u{feff}".as_bytes().to_vec();
        bytes.extend(serde_json::to_vec(&data).unwrap());
        let mut sdh = typed("all-tracks", 4, SubtitleKind::Sdh);
        sdh.note = Some("cues".to_string());
        assert_eq!(
            load(&tree_with(&bytes), &PathKey::new("disc-b/subtitle-labels.json")),
            Ok(vec![typed("t1", 2, SubtitleKind::Regular), sdh])
        );
    }

    #[test]
    fn load_refuses_a_file_that_is_not_json_or_has_problems_and_names_it() {
        let path = PathKey::new("disc-b/subtitle-labels.json");
        let broken = load(&tree_with(b"{"), &path).unwrap_err();
        assert!(broken.starts_with("corpus/owner/disc-b/subtitle-labels.json: not valid JSON ("), "{broken}");
        assert!(broken.ends_with(')'), "{broken}");
        let data = with(file(vec![with(label("t1", 2, "regular"), "sure", json!("yes"))]), "slug", json!("disc-c"));
        assert_eq!(
            load(&tree_with(&serde_json::to_vec(&data).unwrap()), &path),
            Err(concat!(
                "corpus/owner/disc-b/subtitle-labels.json: slug is 'disc-c' but the file is in corpus/owner/disc-b; ",
                "track t1 stream 2: sure must be true or false",
            )
            .to_string())
        );
    }

    #[test]
    fn set_adds_a_label_and_sorts_the_tracks_by_label_then_stream() {
        let data = file(vec![label("t1", 2, "sdh"), label("all-tracks", 10, "sdh")]);
        let (written, left) = set_track(data, &typed("all-tracks", 4, SubtitleKind::Regular)).unwrap();
        assert_eq!(written, file(vec![label("all-tracks", 4, "regular"), label("all-tracks", 10, "sdh"), label("t1", 2, "sdh")]));
        assert_eq!(left, Vec::<String>::new());
    }

    #[test]
    fn set_with_the_same_kind_how_sure_saw_verdict_note_and_style_keeps_the_file_as_it_is() {
        let data = file(vec![label("t1", 2, "sdh"), label("all-tracks", 4, "regular")]);
        let mut again = typed("all-tracks", 4, SubtitleKind::Regular);
        again.date = "2026-10-20".to_string();
        let (written, left) = set_track(data.clone(), &again).unwrap();
        assert_eq!(written, data);
        assert_eq!(written["tracks"][0]["label"], json!("t1"));
        assert_eq!(written["tracks"][1]["date"], json!("2026-10-14"));
        assert_eq!(left, Vec::<String>::new());
    }

    #[test]
    fn set_replaces_a_label_and_keeps_every_old_one_in_its_history() {
        let earlier = with(label("all-tracks", 4, "other"), "replaced", json!("2026-10-10"));
        let data = file(vec![with(label("all-tracks", 4, "regular"), "history", json!([earlier.clone()]))]);
        let new = SubtitleLabel {
            label: "all-tracks".to_string(),
            stream: 4,
            kind: SubtitleKind::Sdh,
            how: "watched".to_string(),
            sure: false,
            date: "2026-10-15".to_string(),
            saw_verdict: false,
            note: Some("cues in brackets".to_string()),
            style: None,
        };
        let (written, left) = set_track(data, &new).unwrap();
        let expected = json!({
            "label": "all-tracks", "stream": 4, "kind": "sdh", "how": "watched", "sure": false, "date": "2026-10-15",
            "saw_verdict": false, "note": "cues in brackets", "style": null,
            "history": [earlier, with(label("all-tracks", 4, "regular"), "replaced", json!("2026-10-15"))],
        });
        assert_eq!(written["tracks"], json!([expected]));
        assert_eq!(left, Vec::<String>::new());
        // A new note alone is a new label.
        let mut noted = typed("all-tracks", 4, SubtitleKind::Regular);
        noted.note = Some("plain".to_string());
        let (written, _) = set_track(file(vec![label("all-tracks", 4, "regular")]), &noted).unwrap();
        assert_eq!(written["tracks"][0]["note"], json!("plain"));
        assert_eq!(written["tracks"][0]["history"], json!([with(label("all-tracks", 4, "regular"), "replaced", json!("2026-10-14"))]));
        // So is a verdict seen before the label (row 14 (a)).
        let mut after = typed("all-tracks", 4, SubtitleKind::Regular);
        after.saw_verdict = true;
        let (written, _) = set_track(file(vec![label("all-tracks", 4, "regular")]), &after).unwrap();
        assert_eq!(written["tracks"][0]["saw_verdict"], json!(true));
        assert_eq!(written["tracks"][0]["history"], json!([with(label("all-tracks", 4, "regular"), "replaced", json!("2026-10-14"))]));
        // And so is a UK style (RP25: row 14's UK-style SDH track).
        let mut uk = typed("all-tracks", 4, SubtitleKind::Sdh);
        uk.style = Some("uk".to_string());
        let (written, _) = set_track(file(vec![label("all-tracks", 4, "sdh")]), &uk).unwrap();
        assert_eq!(written["tracks"][0]["style"], json!("uk"));
        assert_eq!(written["tracks"][0]["history"], json!([with(label("all-tracks", 4, "sdh"), "replaced", json!("2026-10-14"))]));
    }

    #[test]
    fn set_refuses_a_change_that_adds_a_problem() {
        let data = file(vec![label("t1", 2, "sdh")]);
        assert_eq!(
            set_track(data.clone(), &typed("All", 2, SubtitleKind::Sdh)),
            Err("not written: track All stream 2: label is not a rip label".to_string())
        );
        let mut bad_date = typed("all-tracks", 4, SubtitleKind::Sdh);
        bad_date.date = "2026-13-01".to_string();
        assert_eq!(
            set_track(data.clone(), &bad_date),
            Err("not written: track all-tracks stream 4: date '2026-13-01' is not YYYY-MM-DD".to_string())
        );
        assert_eq!(
            set_track(data.clone(), &typed("all-tracks", -1, SubtitleKind::Sdh)),
            Err("not written: track all-tracks stream -1: stream is not ffprobe's stream number (an integer, 0 or more)".to_string())
        );
        let mut audio_how = typed("all-tracks", 4, SubtitleKind::Sdh);
        audio_how.how = "listened".to_string();
        assert_eq!(
            set_track(data.clone(), &audio_how),
            Err("not written: track all-tracks stream 4: how 'listened' is not one of read, watched, packaging, menu, flag, player, thediscdb"
                .to_string())
        );
        let mut british = typed("all-tracks", 4, SubtitleKind::Sdh);
        british.style = Some("british".to_string());
        assert_eq!(
            set_track(data, &british),
            Err("not written: track all-tracks stream 4: style 'british' is not null or 'uk'".to_string())
        );
    }

    #[test]
    fn set_returns_the_problems_the_file_already_had_and_may_clear_them() {
        let data = file(vec![with(label("t1", 2, "sdh"), "sure", json!("yes"))]);
        let (written, left) = set_track(data, &typed("all-tracks", 4, SubtitleKind::Regular)).unwrap();
        assert_eq!(left, ["track t1 stream 2: sure must be true or false"]);
        let (written, left) = set_track(written, &typed("t1", 2, SubtitleKind::Sdh)).unwrap();
        assert_eq!(left, Vec::<String>::new());
        assert_eq!(written["tracks"][1]["label"], json!("t1"));
        assert_eq!(written["tracks"][1]["history"][0]["sure"], json!("yes"));
    }

    #[test]
    fn an_edit_needs_a_file_with_a_tracks_list() {
        let no_tracks = "the file has no 'tracks' list, so nothing is changed".to_string();
        for data in [json!({"format": FORMAT, "slug": "disc-b"}), json!([]), with(file(vec![]), "tracks", json!({}))] {
            assert_eq!(set_track(data.clone(), &typed("all-tracks", 4, SubtitleKind::Sdh)), Err(no_tracks.clone()));
            assert_eq!(unset_track(data, "all-tracks", 4, "wrong stream", "2026-10-16"), Err(no_tracks.clone()));
        }
    }

    #[test]
    fn unset_moves_every_label_of_the_track_to_removed_with_why_and_when() {
        let data = file(vec![label("all-tracks", 4, "sdh"), label("t1", 2, "regular"), label("all-tracks", 4, "regular")]);
        let (written, left) = unset_track(data, "all-tracks", 4, "wrong stream", "2026-10-16").unwrap();
        let moved = |kind: &str| with(with(label("all-tracks", 4, kind), "removed", json!("2026-10-16")), "why", json!("wrong stream"));
        assert_eq!(written["tracks"], json!([label("t1", 2, "regular")]));
        assert_eq!(written["removed"], json!([moved("sdh"), moved("regular")]));
        let keys: Vec<&str> = written.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["format", "slug", "tracks", "removed"]);
        // The file had the track labelled twice; that problem is gone with the labels.
        assert_eq!(left, Vec::<String>::new());
    }

    #[test]
    fn unset_appends_to_the_removed_list_the_file_has() {
        let earlier = with(with(label("all-tracks", 6, "forced"), "removed", json!("2026-10-15")), "why", json!("an empty stream"));
        let data = with(file(vec![label("all-tracks", 4, "sdh")]), "removed", json!([earlier.clone()]));
        let (written, left) = unset_track(data, "all-tracks", 4, "wrong stream", "2026-10-16").unwrap();
        let moved = with(with(label("all-tracks", 4, "sdh"), "removed", json!("2026-10-16")), "why", json!("wrong stream"));
        assert_eq!(written["removed"], json!([earlier, moved]));
        assert_eq!(written["tracks"], json!([]));
        assert_eq!(left, Vec::<String>::new());
    }

    #[test]
    fn unset_refuses_a_track_without_a_label_or_without_a_reason() {
        let data = file(vec![label("t1", 2, "regular")]);
        assert_eq!(unset_track(data.clone(), "t1", 9, "wrong stream", "2026-10-16"), Err("t1 stream 9 has no label".to_string()));
        assert_eq!(unset_track(data.clone(), "eng-only", 2, "wrong stream", "2026-10-16"), Err("eng-only stream 2 has no label".to_string()));
        assert_eq!(unset_track(data, "t1", 2, "", "2026-10-16"), Err("unset needs --note: why the label is taken away".to_string()));
    }

    #[test]
    fn unset_refuses_a_removed_value_that_is_not_a_list() {
        let data = with(file(vec![label("t1", 2, "regular")]), "removed", json!("none"));
        assert_eq!(
            unset_track(data, "t1", 2, "wrong stream", "2026-10-16"),
            Err("'removed' is not a list, so the label cannot be moved there".to_string())
        );
    }

    #[test]
    fn unset_refuses_a_date_that_adds_a_problem() {
        let data = file(vec![label("t1", 2, "regular")]);
        assert_eq!(
            unset_track(data, "t1", 2, "wrong stream", "yesterday"),
            Err("not written: removed 0: removed 'yesterday' is not YYYY-MM-DD".to_string())
        );
    }
}
