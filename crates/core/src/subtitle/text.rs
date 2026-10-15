//! The text rule for English subtitle tracks (design spec 8.7, experiment 14; EXP-14-design.md section 3): the port
//! of the phase 0 Python `subtitle_text` of commit 75fa76a's tree, translated in its loop order. Every threshold is
//! in `Rule` and is PROVISIONAL.
//!
//! `core` reads nothing: `load_title` and `subs_run` take the recording and the OCR text as trees (port spec 3.2).
//! Event lines and event words hold subtitle text in memory only; the lines this module prints hold numbers,
//! classes and decisions only, and `SubsJson` holds no line, word or word pair.
//!
//! Exactness (port spec section 4):
//! - every `round(x, 3)` and `round(x, 4)` of the Python is `py_round` (half to even on the exact binary value);
//! - every pattern runs on folded text, which is ASCII. Python's `\s` also matches `\x1c` to `\x1f` (`str.isspace`)
//!   where the Rust crates' `\s` does not, so the patterns spell it `[\s\x1C-\x1F]` and `\S` as `[^\s\x1C-\x1F]`;
//!   `str.isalpha` and `str.lower` on ASCII are `is_ascii_alphabetic` and `to_ascii_lowercase`;
//! - `bisect`, `sorted`, `min` and `max` keep their tie rules (stable sorts; the first of equals wins);
//! - spec87 iterates its dialogue group in CPython 3.9's set order (`PySet`), which decides SDH against regular when
//!   the group's two tracks have the same number of events.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use unicode_normalization::char::canonical_combining_class;
use unicode_normalization::UnicodeNormalization;

use crate::recording::{py_fixed, py_strip, Ordered};

#[cfg(test)]
mod tests;

// ------------------------------------------------------------------ the thresholds

/// The rule's thresholds with the Python's defaults (all PROVISIONAL); the field names are RULE's keys. Tests change
/// one with struct update syntax (`Rule { SDH_SO: 0, ..Rule::default() }`, the Python's mock.patch.dict).
/// Py: subtitle_text.RULE.
#[allow(non_snake_case)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    /// sub_clear_max_bytes (spec 8.7): larger packets are displayed events.
    pub CLEAR_MAX_BYTES: i64,
    /// subs_tolerance_ms (spec 8.7), in seconds.
    pub TOL_S: f64,
    /// subs_same_min: timing says same group.
    pub SAME_MIN: f64,
    /// subs_diff_max: timing says different groups.
    pub DIFF_MAX: f64,
    /// A full track has at least this share of the largest English track's events.
    pub FULL_SHARE: f64,
    /// 8.7: a separately authored forced track has fewer events.
    pub FORCED_MAX_EVENTS: i64,
    /// Readability: share of events with 2 or more letters.
    pub READ_LETTER_EVENTS: f64,
    /// Share of word-like tokens (note stand-ins and '|' not counted).
    pub READ_WORDLIKE: f64,
    pub READ_MIN_WORDS: i64,
    /// A cue counts only in an event whose own tokens are at least this word-like.
    pub EVENT_WORDLIKE: f64,
    /// Language (stopword shares on ASCII-folded words).
    pub LANG_MIN_WORDS: i64,
    /// English when the English share is at least this.
    pub EN_MIN: f64,
    /// Another language when English is at most this ...
    pub EN_MAX_OTHER: f64,
    /// ... and that language's share is at least this ...
    pub OTHER_MIN: f64,
    /// ... and at least this many times the English share.
    pub OTHER_RATIO: f64,
    /// SDH evidence on one track: share of capital lines at which the whole track is in capitals.
    pub CAPS_STYLE: f64,
    /// Marker events (cue, label or '>>').
    pub SDH_M: i64,
    /// ... per event.
    pub SDH_RATE: f64,
    /// Sound-only events (every letter inside a cue or label).
    pub SDH_SO: i64,
    /// Events with a cue that names a sound.
    pub SDH_CS: i64,
    /// At most this many marker events: no SDH evidence.
    pub REG_M_MAX: i64,
    /// More events with damaged brackets than this: not placed.
    pub ODD_MAX: i64,
    /// More capital sound lines without brackets than this: not placed (UK style?).
    pub CAPS_CUE_MAX: i64,
    /// Pair: capital-only events with no counterpart (reported; review only).
    pub UC_MIN: i64,
    /// Content of two tracks (time-free bigram containment).
    pub CB_MIN_BIGRAMS: i64,
    /// Both directions at least this: same words.
    pub CB_SAME: f64,
    /// Both directions at most this: other words.
    pub CB_DIFF: f64,
    /// Subset: the smaller track's bigrams found in the larger at least this ...
    pub CB_SUBSET_IN: f64,
    /// ... and the larger's found in the smaller under this ...
    pub CB_SUBSET_BACK: f64,
    /// ... and the smaller has at most this share of the larger's events.
    pub SUBSET_EVENTS: f64,
    /// Forced candidate confirmed: its bigrams found in the dialogue track.
    pub FORCED_CB: f64,
    /// Forced candidate with this share of note-line events: lyrics, review.
    pub NOTE_LINES: f64,
}

impl Default for Rule {
    fn default() -> Rule {
        Rule {
            CLEAR_MAX_BYTES: 64,
            TOL_S: 0.300,
            SAME_MIN: 0.70,
            DIFF_MAX: 0.30,
            FULL_SHARE: 0.10,
            FORCED_MAX_EVENTS: 150,
            READ_LETTER_EVENTS: 0.90,
            READ_WORDLIKE: 0.85,
            READ_MIN_WORDS: 20,
            EVENT_WORDLIKE: 0.50,
            LANG_MIN_WORDS: 50,
            EN_MIN: 0.20,
            EN_MAX_OTHER: 0.08,
            OTHER_MIN: 0.12,
            OTHER_RATIO: 3.0,
            CAPS_STYLE: 0.50,
            SDH_M: 5,
            SDH_RATE: 0.010,
            SDH_SO: 3,
            SDH_CS: 2,
            REG_M_MAX: 0,
            ODD_MAX: 2,
            CAPS_CUE_MAX: 2,
            UC_MIN: 3,
            CB_MIN_BIGRAMS: 20,
            CB_SAME: 0.50,
            CB_DIFF: 0.20,
            CB_SUBSET_IN: 0.60,
            CB_SUBSET_BACK: 0.50,
            SUBSET_EVENTS: 0.70,
            FORCED_CB: 0.60,
            NOTE_LINES: 0.30,
        }
    }
}

// ------------------------------------------------------------------ Python's spellings

/// round(x, ndigits) as Python 3.9 rounds a float (float___round__): a NaN or an infinity, or more digits than a
/// double has, is x itself; otherwise the exact decimal expansion of x rounded half to even to `ndigits` places
/// (`_Py_dg_dtoa` mode 3, the digits '%.Nf' prints, here `py_fixed`), read back as the nearest double. Every
/// round(x, 3) and round(x, 4) of the module goes through it, because those values meet thresholds. Negative
/// `ndigits` (rounding to tens) is not used by the module and panics. Py: round.
pub fn py_round(x: f64, ndigits: i32) -> f64 {
    // NDIGITS_MAX of floatobject.c: (DBL_MANT_DIG - DBL_MIN_EXP) * 0.30103, truncated.
    if !x.is_finite() || ndigits > 323 {
        return x;
    }
    assert!(ndigits >= 0, "py_round: negative ndigits are not used by subtitle_text");
    py_fixed(x, ndigits as usize).parse::<f64>().expect("'%.Nf' of a finite float reads back")
}

/// Python's titlecase letters (general category Lt, Unicode 13): `str.islower` and `str.isupper` refuse a string
/// holding one. Py: Py_UNICODE_ISTITLE.
fn is_titlecase(c: char) -> bool {
    matches!(
        c,
        '\u{01C5}'
            | '\u{01C8}'
            | '\u{01CB}'
            | '\u{01F2}'
            | '\u{1F88}'..='\u{1F8F}'
            | '\u{1F98}'..='\u{1F9F}'
            | '\u{1FA8}'..='\u{1FAF}'
            | '\u{1FBC}'
            | '\u{1FCC}'
            | '\u{1FFC}'
    )
}

/// str.islower: no uppercase or titlecase character and at least one lowercase one. Python's lowercase and uppercase
/// are the Unicode derived properties Lowercase and Uppercase, which `char::is_lowercase` and `char::is_uppercase`
/// test. Py: unicode_islower_impl.
fn py_islower(s: &str) -> bool {
    let mut cased = false;
    for c in s.chars() {
        if c.is_uppercase() || is_titlecase(c) {
            return false;
        }
        if !cased && c.is_lowercase() {
            cased = true;
        }
    }
    cased
}

/// str.isupper: no lowercase or titlecase character and at least one uppercase one. Py: unicode_isupper_impl.
fn py_isupper(s: &str) -> bool {
    let mut cased = false;
    for c in s.chars() {
        if c.is_lowercase() || is_titlecase(c) {
            return false;
        }
        if !cased && c.is_uppercase() {
            cased = true;
        }
    }
    cased
}

// ------------------------------------------------------------------ CPython's set order

/// CPython 3.9's set table constants (Objects/setobject.c).
const SET_MINSIZE: usize = 8;
const LINEAR_PROBES: usize = 9;
const PERTURB_SHIFT: u32 = 5;

/// A set of stream numbers laid out as CPython 3.9's set object lays out non-negative ints (hash(n) == n; nothing is
/// ever removed), so that iterating it gives the Python's order. Only spec87 needs it: it sorts its dialogue group's
/// members by event count with a stable sort, so two members with the same count keep the set's order.
/// Py: set_add_entry, set_insert_clean, set_table_resize, set_merge, set_copy and set_or of setobject.c (3.9).
#[derive(Clone, Debug)]
struct PySet {
    table: Vec<Option<i64>>,
    fill: usize,
    used: usize,
}

impl PySet {
    /// set(): an empty table of SET_MINSIZE slots.
    fn empty() -> PySet {
        PySet { table: vec![None; SET_MINSIZE], fill: 0, used: 0 }
    }

    /// `{n}`: BUILD_SET adds its one item to an empty set.
    fn singleton(n: i64) -> PySet {
        let mut set = PySet::empty();
        set.add(n);
        set
    }

    fn mask(&self) -> usize {
        self.table.len() - 1
    }

    /// hash(n) of a non-negative int below 2**61 - 1 is n.
    fn hash(key: i64) -> usize {
        assert!(key >= 0, "stream numbers are not negative");
        key as usize
    }

    /// set_insert_clean: the first empty slot of the probe sequence (no key equal to `key` is in the table).
    fn insert_clean(table: &mut [Option<i64>], key: i64) {
        let mask = table.len() - 1;
        let hash = PySet::hash(key);
        let mut perturb = hash;
        let mut i = hash & mask;
        loop {
            if table[i].is_none() {
                table[i] = Some(key);
                return;
            }
            if i + LINEAR_PROBES <= mask {
                for j in 1..=LINEAR_PROBES {
                    if table[i + j].is_none() {
                        table[i + j] = Some(key);
                        return;
                    }
                }
            }
            perturb >>= PERTURB_SHIFT;
            i = (i * 5 + 1 + perturb) & mask;
        }
    }

    /// set_table_resize: the smallest power of two above `minused` (at least SET_MINSIZE), every key reinserted in
    /// the old table's slot order.
    fn resize(&mut self, minused: usize) {
        let mut size = SET_MINSIZE;
        while size <= minused {
            size <<= 1;
        }
        let old = std::mem::replace(&mut self.table, vec![None; size]);
        for key in old.into_iter().flatten() {
            PySet::insert_clean(&mut self.table, key);
        }
        self.fill = self.used;
    }

    /// set_add_entry: nothing when `key` is in the probe sequence, else the first empty slot of it, then a resize
    /// to used * 4 once fill * 5 >= mask * 3.
    fn add(&mut self, key: i64) {
        let mask = self.mask();
        let hash = PySet::hash(key);
        let mut perturb = hash;
        let mut i = hash & mask;
        let slot = 'probe: loop {
            match self.table[i] {
                None => break 'probe i,
                Some(k) if k == key => return,
                Some(_) => {}
            }
            if i + LINEAR_PROBES <= mask {
                for j in 1..=LINEAR_PROBES {
                    match self.table[i + j] {
                        None => break 'probe i + j,
                        Some(k) if k == key => return,
                        Some(_) => {}
                    }
                }
            }
            perturb >>= PERTURB_SHIFT;
            i = (i * 5 + 1 + perturb) & mask;
        };
        self.table[slot] = Some(key);
        self.fill += 1;
        self.used += 1;
        if self.fill * 5 >= mask * 3 {
            let minused = if self.used > 50_000 { self.used * 2 } else { self.used * 4 };
            self.resize(minused);
        }
    }

    /// set_merge (`update` with another set): one resize first when the keys may not fit, then the table copied
    /// as it is, or inserted clean in the other's slot order into an empty table, or added one by one.
    fn merge(&mut self, other: &PySet) {
        if other.used == 0 {
            return;
        }
        if (self.fill + other.used) * 5 >= self.mask() * 3 {
            self.resize((self.used + other.used) * 2);
        }
        if self.fill == 0 && self.mask() == other.mask() && other.fill == other.used {
            self.table = other.table.clone();
            self.fill = other.fill;
            self.used = other.used;
            return;
        }
        if self.fill == 0 {
            self.fill = other.used;
            self.used = other.used;
            for key in other.table.iter().flatten() {
                PySet::insert_clean(&mut self.table, *key);
            }
            return;
        }
        for key in other.table.iter().flatten() {
            self.add(*key);
        }
    }

    /// set_copy: a new set merged from this one. `a | a` is this copy (set_or returns it when both are one object).
    fn copy(&self) -> PySet {
        let mut set = PySet::empty();
        set.merge(self);
        set
    }

    /// `self | other` for two different set objects (set_or): a copy of self updated with other.
    fn union(&self, other: &PySet) -> PySet {
        let mut set = self.copy();
        set.merge(other);
        set
    }

    /// The keys in slot order. Py: iter(set).
    fn iter(&self) -> impl Iterator<Item = i64> + '_ {
        self.table.iter().flatten().copied()
    }

    fn len(&self) -> usize {
        self.used
    }

    fn contains(&self, key: i64) -> bool {
        self.iter().any(|k| k == key)
    }
}

// ------------------------------------------------------------------ folding

/// FOLD_MAP: the music notes and the pilcrow become '#', the note stand-in, so that an engine that ever returns a
/// note is read the same way as one that returns '#'. The inverted ? and ! are dropped: an ASCII-only engine cannot
/// read them, and Tesseract's English model turns them into other characters anyway. Py: FOLD_MAP.
fn fold_map(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{2032}' | '\u{00B4}' | '\u{0060}' | '\u{02BC}' => "'",
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{00AB}' | '\u{00BB}' | '\u{2033}' => "\"",
        '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}' | '\u{2212}' => "-",
        '\u{2026}' => "...",
        '\u{00A0}' => " ",
        '\u{2669}' | '\u{266A}' | '\u{266B}' | '\u{266C}' | '\u{266D}' | '\u{266F}' | '\u{00B6}' => "#",
        '\u{00BF}' | '\u{00A1}' => "",
        '\u{00DF}' => "ss",
        '\u{00E6}' => "ae",
        '\u{00C6}' => "AE",
        '\u{0153}' => "oe",
        '\u{0152}' => "OE",
        '\u{00F8}' => "o",
        '\u{00D8}' => "O",
        '\u{0142}' => "l",
        '\u{0141}' => "L",
        _ => return None,
    })
}

/// ASCII text: FOLD_MAP, then NFKD without characters of non-zero canonical combining class
/// (`unicodedata.combining`), then every other non-ASCII character dropped. Py: fold.
pub fn fold(s: &str) -> String {
    let mut mapped = String::with_capacity(s.len());
    for c in s.chars() {
        match fold_map(c) {
            Some(to) => mapped.push_str(to),
            None => mapped.push(c),
        }
    }
    mapped.nfkd().filter(|&c| canonical_combining_class(c) == 0 && (c as u32) < 128).collect()
}

// ------------------------------------------------------------------ lines and OCR corrections

/// Lines of the proof of concept's text, whose lines are joined with ' | ' while Tesseract reads a capital I as
/// '|'. With the frame's true line count, the extra bare '|' tokens are taken as I, preferring the text start, a
/// '|' after another '|' and a '|' before a lower-case word; without it (`nlines` None or 0), every ' | ' is a line
/// break. `nxt[:1] in '-(['` is true for an empty next token too (the empty string is in every string), which the
/// score keeps. Py: split_joined.
pub fn split_joined(text: &str, nlines: Option<i64>) -> Vec<String> {
    let toks: Vec<&str> = text.split(' ').collect();
    let bare: Vec<usize> = toks.iter().enumerate().filter(|(_, tok)| **tok == "|").map(|(i, _)| i).collect();
    let n_i: usize = match nlines {
        None | Some(0) => 0,
        Some(n) => (bare.len() as i64 - (n - 1)).max(0) as usize,
    };
    let score = |i: usize| -> i64 {
        let nxt = toks.get(i + 1).copied().unwrap_or("");
        let first: String = nxt.chars().take(1).collect();
        let mut s = if i == 0 { 3 } else { 0 };
        if i > 0 && toks[i - 1] == "|" {
            s += 1;
        }
        if py_islower(&first) || first == "'" {
            s += 2;
        }
        if py_isupper(&first) || first.is_empty() || matches!(first.as_str(), "-" | "(" | "[") {
            s -= 1;
        }
        s
    };
    let mut ranked = bare.clone();
    ranked.sort_by_key(|&i| (-score(i), i));
    let as_i: BTreeSet<usize> = ranked.into_iter().take(n_i).collect();
    let mut lines: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for (i, &tok) in toks.iter().enumerate() {
        if tok == "|" && !as_i.contains(&i) {
            lines.push(cur.join(" "));
            cur = Vec::new();
        } else {
            cur.push(tok);
        }
    }
    lines.push(cur.join(" "));
    lines.iter().map(|x| py_strip(x)).filter(|x| !x.is_empty()).map(str::to_string).collect()
}

/// The seven corrections for English text from Tesseract (I read as '|', 'l' or '1'), applied to English-tagged
/// streams in this order: (name, pattern, replacement). They use look-behind and look-ahead, so they run on
/// `fancy_regex`; `[\s\x1C-\x1F]` is Python's `\s` (see the module notes). Py: CORRECTIONS.
const CORRECTION_PATTERNS: [(&str, &str, &str); 7] = [
    // "|", "|!", "|'m"
    ("pipe_I", r"(?<![A-Za-z0-9])\|(?=[\s\x1C-\x1F'.,?!]|$)", "I"),
    // "|t's"
    ("pipe_word_I", r"(?<![A-Za-z0-9])\|(?=[a-z])", "I"),
    // "he|lo"
    ("pipe_l", r"(?<=[a-z])\|(?=[a-z])", "l"),
    // "BORlS", "WHlSTLlNG"
    ("caps_I", r"(?<=[A-Z])[l1|](?=[A-Z])", "I"),
    ("caps_O", r"(?<=[A-Z])0(?=[A-Z])", "O"),
    ("l1_pronoun", r"(?<![A-Za-z0-9'])[l1](?='(?:m|ll|d|ve)\b)", "I"),
    // "lam", "1am"
    ("i_am", r"(?<![A-Za-z0-9])[|l1]am\b", "I am"),
];

/// The counter correct() adds after the seven: the '|' left, each turned into 'l'.
const PIPE_LEFT: &str = "pipe_left";

/// (name, pattern, replacement) of the seven corrections, compiled once. Py: CORRECTIONS.
pub fn corrections() -> &'static [(&'static str, fancy_regex::Regex, &'static str)] {
    static COMPILED: LazyLock<Vec<(&'static str, fancy_regex::Regex, &'static str)>> = LazyLock::new(|| {
        CORRECTION_PATTERNS
            .iter()
            .map(|&(name, pattern, rep)| (name, fancy_regex::Regex::new(pattern).expect("a correction compiles"), rep))
            .collect()
    });
    COMPILED.as_slice()
}

/// re.subn(pattern, rep, text): one pass over the original text, every non-overlapping match replaced, and the
/// number of replacements (fancy_regex's replace_all with a counting replacer). Py: rx.subn.
fn subn(rx: &fancy_regex::Regex, rep: &str, text: &str) -> (String, usize) {
    let mut k = 0usize;
    let out = rx
        .replace_all(text, |_: &fancy_regex::Captures<'_>| {
            k += 1;
            rep
        })
        .into_owned();
    (out, k)
}

/// The seven corrections in order, each counted under its name as re.subn counts, then the '|' left counted as
/// pipe_left and turned into 'l'. `counts` gets every name on every call, in that order. Py: correct.
pub fn correct(line: &str, counts: &mut Ordered<usize>) -> String {
    let mut line = line.to_string();
    for (name, rx, rep) in corrections() {
        let (fixed, k) = subn(rx, rep, &line);
        line = fixed;
        let total = counts.get(name).copied().unwrap_or(0) + k;
        counts.insert(name.to_string(), total);
    }
    let k = line.matches('|').count();
    let total = counts.get(PIPE_LEFT).copied().unwrap_or(0) + k;
    counts.insert(PIPE_LEFT.to_string(), total);
    line.replace('|', "l")
}

// ------------------------------------------------------------------ markers (on folded, corrected text)

/// Compiles one marker pattern with the `regex` crate, which has no look-around: compiling is the proof that the
/// pattern needs none (port spec 3.3).
fn marker(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("a marker pattern compiles without look-around")
}

/// A cue: a bracketed or parenthesised span of 2 to 80 characters, with at least 2 letters and only letters, digits,
/// spaces and light punctuation inside (OCR junk does not make one). Either bracket may close the other. Py: RE_CUE.
static RE_CUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    marker(r"[\[(][\s\x1C-\x1F]*([A-Za-z0-9][A-Za-z0-9 ',.!?&:;/\-]{0,78}[A-Za-z0-9.!?'])[\s\x1C-\x1F]*[\])]")
});
/// A capital speaker label at the start of a line, after an optional dash, followed by text on the same line.
/// Py: RE_LABEL (used with .match: the pattern starts with ^).
static RE_LABEL: LazyLock<regex::Regex> = LazyLock::new(|| {
    marker(
        r"^[\s\x1C-\x1F]*(?:-[\s\x1C-\x1F]*)?([A-Z][A-Z0-9'.\-]*(?:[\s\x1C-\x1F]+[A-Z0-9'.\-]+){0,2})[\s\x1C-\x1F]*:[\s\x1C-\x1F]+[^\s\x1C-\x1F]",
    )
});
/// Py: RE_CHEV (used with .match).
static RE_CHEV: LazyLock<regex::Regex> = LazyLock::new(|| marker(r"^[\s\x1C-\x1F]*>>"));
/// Lyrics (note stand-in, strict): '#' or '*' at either end of a line, after folding (real notes are '#').
/// Py: RE_NOTE (used with .search).
static RE_NOTE: LazyLock<regex::Regex> =
    LazyLock::new(|| marker(r"^[\s\x1C-\x1F]*[#*]|[#*][\s\x1C-\x1F]*$"));
/// Lyrics (loose, ENGINE-DEPENDENT, reported only): a lone token reported as a Tesseract misread of a note.
/// Py: RE_NOTE_LOOSE (used with .search).
static RE_NOTE_LOOSE: LazyLock<regex::Regex> = LazyLock::new(|| {
    marker(
        r"^[\s\x1C-\x1F]*(?:J|JJ|Jf|Ji|Jl|P|D|d|f|\$|%)[\s\x1C-\x1F]+[^\s\x1C-\x1F]|[^\s\x1C-\x1F][\s\x1C-\x1F]+(?:J|JJ|Jf|Ji|Jl|P|D|d|f|\$|%)[\s\x1C-\x1F]*$",
    )
});
/// Damaged brackets, on the folded text before corrections: a line wrapped in '|' or '{' '}' (a misread bracket
/// pair), counted per event together with unbalanced () and []. Py: RE_WRAP (used with .match).
static RE_WRAP: LazyLock<regex::Regex> = LazyLock::new(|| {
    marker(
        r"^[\s\x1C-\x1F]*-?[\s\x1C-\x1F]*[|{][\s\x1C-\x1F]*[A-Za-z][^|{}()\[\]]*[A-Za-z.!?][\s\x1C-\x1F]*[|}][\s\x1C-\x1F]*$",
    )
});
/// Py: RE_WORD (used with .findall; its one group does not capture).
static RE_WORD: LazyLock<regex::Regex> = LazyLock::new(|| marker(r"[a-z]+(?:'[a-z]+)?"));
/// Py: RE_WORDLIKE, used with .fullmatch, so anchored here at both ends.
static RE_WORDLIKE: LazyLock<regex::Regex> = LazyLock::new(|| {
    marker(r#"^(?:["'(\[-]*(?:[A-Za-z]+(?:[-'][A-Za-z]+)*|\d+(?:[.,:]\d+)*)["'.,!?:;)\]-]*)$"#)
});

/// STOP's word lists before its two adjustments, in its order. Py: STOP's literals.
const STOP_LISTS: [(&str, &str); 4] = [
    (
        "en",
        "the and is are was were be been to of in on at for with that this it you i he she we they my your me \
him her not do don't what have has will just so if there here can all get go know it's i'm you're that's would \
could did didn't can't from about out up yes oh well right okay",
    ),
    (
        "es",
        "el la los las de del que y en un una es se lo le por con para su al te mi tu yo pero como mas esta \
este eso esto si ya hay muy bien nos os usted senor ella ellos aqui nada todo cuando donde porque tengo tiene \
puede vamos",
    ),
    (
        "fr",
        "le les des du et est je tu il elle nous vous ils que qui ne pas pour dans sur avec ce cette mais ou mon \
ma oui non c'est j'ai tout bien ca moi toi lui suis sont fait rien ici",
    ),
    (
        "de",
        "der die das und ist ich du er sie wir ihr nicht ein eine zu mit auf fur den dem des es ja nein was wie \
aber auch noch mir mich dich hier sein haben bin bist wird kann",
    ),
];

/// English words that are common Spanish words, taken out of English. Py: the set subtracted from STOP['en'].
const NOT_ENGLISH: [&str; 10] = ["me", "he", "has", "a", "no", "son", "come", "sin", "con", "pan"];

/// The stopword sets in STOP's order (en, es, fr, de): a word in two lists counts for neither, and English loses
/// NOT_ENGLISH. Py: STOP after its two adjustments.
static STOP: LazyLock<Vec<(&'static str, BTreeSet<&'static str>)>> = LazyLock::new(|| {
    let mut langs_of: BTreeMap<&'static str, BTreeSet<&'static str>> = BTreeMap::new();
    for (lang, words) in STOP_LISTS {
        for word in words.split_whitespace() {
            langs_of.entry(word).or_default().insert(lang);
        }
    }
    STOP_LISTS
        .iter()
        .map(|&(lang, words)| {
            let mut set: BTreeSet<&'static str> =
                words.split_whitespace().filter(|word| langs_of[*word].len() == 1).collect();
            if lang == "en" {
                for word in NOT_ENGLISH {
                    set.remove(word);
                }
            }
            (lang, set)
        })
        .collect()
});

/// One language's stopwords. Py: STOP[lang].
fn stop(lang: &str) -> &'static BTreeSet<&'static str> {
    &STOP.iter().find(|(l, _)| *l == lang).expect("a language of STOP").1
}

/// Py: wordlike (RE_WORDLIKE.fullmatch).
fn wordlike(tok: &str) -> bool {
    RE_WORDLIKE.is_match(tok)
}
// ------------------------------------------------------------------ loading

use crate::recording::files::{OcrLine, Packet};
use crate::recording::{py_float_value, py_str};
use crate::subtitle::align::{self, bisect_left, size_of};

/// Python's `a < b` for two floats as a total order for sorting: -0.0 equals 0.0 (Python's == says so and its
/// stable sort keeps their order); a NaN, which no input holds, sorts after every number instead of breaking the
/// sort. Py: the float comparisons of sorted() and list.sort().
fn py_cmp(a: f64, b: f64) -> std::cmp::Ordering {
    let zero_is_zero = |x: f64| if x == 0.0 { 0.0 } else { x };
    zero_is_zero(a).total_cmp(&zero_is_zero(b))
}

/// A displayed event: start and duration in seconds, and its OCR lines (empty when it has none). Holds subtitle
/// text in `lines`: in memory only. Py: the event dicts.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub start: f64,
    pub dur: f64,
    pub lines: Vec<String>,
}

/// One OCR frame with text: its time, whether that time is a packet time ('pts' in the record), its lines.
/// Py: ocr_records' dicts.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrRecord {
    pub t: f64,
    pub pts: bool,
    pub lines: Vec<String>,
}

/// How pair_text paired: "order" (equal counts) or "time".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairMethod {
    Order,
    Time,
}

impl PairMethod {
    /// The Python's word.
    pub fn as_str(self) -> &'static str {
        match self {
            PairMethod::Order => "order",
            PairMethod::Time => "time",
        }
    }
}

/// Displayed events: packets with a pts_time larger than `clear_max_bytes` (every packet with a size when it is 0;
/// a packet without a size is size 0), duration 0.0 for None, "" or "N/A", sorted by start (stable). Err is the
/// Python's int() or float() text. Py: packet_events.
pub fn packet_events(packets: &[Packet], clear_max_bytes: i64) -> Result<Vec<Event>, String> {
    let mut out = Vec::new();
    for p in packets {
        let Some(pts) = &p.pts_time else {
            continue;
        };
        if size_of(p)? <= clear_max_bytes {
            continue;
        }
        let start = py_float_value(pts)?;
        let dur = match &p.duration_time {
            None | Some(serde_json::Value::Null) => 0.0,
            Some(serde_json::Value::String(text)) if text.is_empty() || text == "N/A" => 0.0,
            Some(other) => py_float_value(other)?,
        };
        out.push(Event { start, dur, lines: Vec::new() });
    }
    out.sort_by(|a, b| py_cmp(a.start, b.start));
    Ok(out)
}

/// (t, lines) for every OCR frame with text. A record with a 'lines' array (the final ocr-subs.ps1) is taken as it
/// is (each item as str(), stripped, blanks dropped); the proof of concept's ' | '-joined text is split with the
/// frame's line count from `counts` (frame -> lines of sub-<n>.frames.json). The time is 'pts', else 't', else 0.0.
/// Never Err today (the records are typed); the Result keeps the reader's signature. Py: ocr_records (after its two
/// reads, which load_title does).
pub fn ocr_records(records: &[OcrLine], counts: &BTreeMap<i64, Option<i64>>) -> Result<Vec<OcrRecord>, String> {
    let mut out = Vec::new();
    for r in records {
        let lines: Vec<String> = match &r.lines {
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .map(|item| py_strip(&py_str(item)).to_string())
                .filter(|line| !line.is_empty())
                .collect(),
            _ => {
                let text = py_strip(r.text.as_deref().unwrap_or(""));
                if text.is_empty() {
                    Vec::new()
                } else {
                    split_joined(text, r.frame.and_then(|frame| counts.get(&frame).copied().flatten()))
                }
            }
        };
        if !lines.is_empty() {
            out.push(OcrRecord { t: r.pts.or(r.t).unwrap_or(0.0), pts: r.pts.is_some(), lines });
        }
    }
    Ok(out)
}

/// Gives each displayed event its OCR lines. Equal counts: by order (the k-th frame with text is the k-th event;
/// true on every real track so far). Otherwise by time: the proof of concept's 't' is the packet time minus the
/// file's first packet time, on a 0.25 s grid; each frame goes to the nearer of the two events around it (the
/// earlier on a tie) when it is within 0.30 s. Returns the method and the number of frames left unmatched.
/// Py: pair_text.
pub fn pair_text(events: &mut [Event], recs: &[OcrRecord]) -> (PairMethod, usize) {
    if recs.len() == events.len() {
        for (e, r) in events.iter_mut().zip(recs) {
            e.lines = r.lines.clone();
        }
        return (PairMethod::Order, 0);
    }
    for e in events.iter_mut() {
        e.lines = Vec::new();
    }
    if events.is_empty() {
        return (PairMethod::Time, recs.len());
    }
    let starts: Vec<f64> = events.iter().map(|e| e.start).collect();
    let off = if recs.first().is_some_and(|r| r.pts) {
        0.0
    } else {
        events[0].start - recs.first().map_or(0.0, |r| r.t)
    };
    let mut left = 0;
    for r in recs {
        let t = r.t + off;
        let i = bisect_left(&starts, t);
        // min((j for j in (i - 1, i) if 0 <= j < len(starts)), key=...): the first of equal distances wins.
        let mut best: Option<usize> = None;
        let candidates = [i.checked_sub(1), Some(i).filter(|&j| j < starts.len())];
        for j in candidates.into_iter().flatten() {
            match best {
                Some(b) if (starts[j] - t).abs() >= (starts[b] - t).abs() => {}
                _ => best = Some(j),
            }
        }
        let best = best.expect("an event lies on one side of every time");
        if (starts[best] - t).abs() <= 0.30 {
            events[best].lines.extend(r.lines.iter().cloned());
        } else {
            left += 1;
        }
    }
    (PairMethod::Time, left)
}
// ------------------------------------------------------------------ per-event helpers

use crate::audio::measures::py_max;
use crate::recording::py_split;

/// max(x, 0.0) as Python evaluates it: x unless 0.0 > x. Py: max(e['dur'], 0.0).
fn py_max0(x: f64) -> f64 {
    py_max([x, 0.0])
}

/// bisect.bisect_right on a list of floats, CPython's loop. Py: bisect.bisect_right.
fn bisect_right(a: &[f64], x: f64) -> usize {
    let (mut lo, mut hi) = (0, a.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if x < a[mid] {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

/// The tokens Tesseract has returned for a note, counted as a note-only event when alone (ENGINE-DEPENDENT).
/// Py: NOTE_LOOSE_TOKENS.
const NOTE_LOOSE_TOKENS: [&str; 11] = ["J", "JJ", "Jf", "Ji", "Jl", "P", "D", "d", "f", "$", "%"];

/// Sound stems (prefixes, ASCII). Used only inside a cue or on a capital line, never on dialogue words: outside them
/// they hit ordinary words (single, pantry) and names. Py: SOUND_STEMS.
const SOUND_STEMS: &str = "laugh chuckl giggl sigh gasp groan grunt scream shout yell whisper cough sniff sob cries \
crying music sing hum whistl ring buzz beep knock thud bang crash slam footstep applau cheer whoop breath pant mumbl \
mutter murmur siren honk bark growl howl throat exhal inhal snor click thunder rumbl static chatter indistinct \
inaudible explosion gunshot gunfire splash screech creak rustl clatter chim grumbl stammer stutter clears squeal \
wheez moan whimper yelp shriek snicker scoff door phone telephone bell engine horn tire brake glass shatter thump \
snap clap kiss wind rain alarm radio yawn weep wail whoosh typing tv television sneez hiccup splutter \
spluttering playing plays";

/// The letters of folded (ASCII) text: str.isalpha on ASCII is [A-Za-z]. Py: letters.
fn letters(s: &str) -> usize {
    s.chars().filter(|c| c.is_ascii_alphabetic()).count()
}

/// A word of the (ASCII) text starts with a sound stem. Py: sound_named.
fn sound_named(s: &str) -> bool {
    RE_WORD
        .find_iter(&s.to_ascii_lowercase())
        .any(|word| SOUND_STEMS.split_whitespace().any(|stem| word.as_str().starts_with(stem)))
}

/// Py: is_note_token.
fn is_note_token(tok: &str) -> bool {
    matches!(tok, "#" | "##" | "*" | "**" | "|") || (!tok.is_empty() && tok.chars().all(|c| c == '#' || c == '*'))
}

// ------------------------------------------------------------------ per-event and per-track measures

/// One event's features. `words` holds subtitle words: in memory only, never serialised. Py: event_features' dict.
#[derive(Clone, Debug, PartialEq)]
pub struct Features {
    pub odd: bool,
    pub note_only: bool,
    pub tokens: usize,
    pub wordlike: usize,
    pub cue: bool,
    pub cue_sound: bool,
    pub label: bool,
    pub chev: bool,
    pub letters: usize,
    pub sound_only: bool,
    pub note: bool,
    pub note_loose: bool,
    pub caps_lines: usize,
    pub caps_cue: bool,
    pub all_caps: bool,
    pub lines: usize,
    pub words: Vec<String>,
}

/// The features of one event from its OCR lines: folded, corrected when the stream is English (else '|' becomes a
/// space), stripped. Py: event_features.
fn event_features(lines_raw: &[String], english: bool, counts: &mut Ordered<usize>, rule: &Rule) -> Features {
    let folded: Vec<String> = lines_raw.iter().map(|line| fold(line)).collect();
    let joined_raw = folded.join(" ");
    let count = |c: char| joined_raw.matches(c).count();
    let odd = count('(') + count('[') != count(')') + count(']') || folded.iter().any(|line| RE_WRAP.is_match(line));
    let corrected: Vec<String> = folded
        .iter()
        .map(|line| if english { correct(line, counts) } else { line.replace('|', " ") })
        .collect();
    let lines: Vec<String> =
        corrected.iter().map(|line| py_strip(line)).filter(|line| !line.is_empty()).map(str::to_string).collect();
    let joined = lines.join(" ");
    let toks: Vec<&str> = py_split(&joined);
    // A note-only event (a music cue read as note stand-ins, or as one stray letter; ENGINE-DEPENDENT) is left out
    // of the readability share of events with letters.
    let note_only = !toks.is_empty()
        && (toks.iter().all(|tok| is_note_token(tok)) || (toks.len() == 1 && NOTE_LOOSE_TOKENS.contains(&toks[0])));
    let real: Vec<&str> = toks.iter().copied().filter(|tok| !is_note_token(tok)).collect();
    let tokens = real.len();
    let wordlike_tokens = real.iter().filter(|tok| wordlike(tok)).count();
    let ok_event = real.is_empty() || (wordlike_tokens as f64) / (real.len() as f64) >= rule.EVENT_WORDLIKE;
    let mut cues: Vec<String> = Vec::new();
    for caps in RE_CUE.captures_iter(&joined) {
        let inner = caps.get(1).map_or("", |m| m.as_str());
        if letters(inner) >= 2 && ok_event {
            cues.push(inner.to_string());
        }
    }
    let cue = !cues.is_empty();
    let cue_sound = cues.iter().any(|c| sound_named(c));
    let mut labels = 0usize;
    let mut rest: Vec<String> = Vec::new();
    for line in &lines {
        let mut l: &str = line;
        if let Some(caps) = RE_LABEL.captures(l) {
            let group = caps.get(1).expect("RE_LABEL has a group 1");
            if letters(group.as_str()) >= 2 {
                labels += 1;
                l = l[group.end()..].trim_start_matches([' ', ':']);
            }
        }
        rest.push(RE_CUE.replace_all(l, " ").into_owned());
    }
    let label = labels > 0;
    let chev = lines.iter().any(|line| RE_CHEV.is_match(line));
    let remainder = RE_CUE.replace_all(&rest.join(" "), " ").into_owned();
    // re.sub(r"[#*>]", ' ', remainder)
    let remainder: String = remainder.chars().map(|c| if matches!(c, '#' | '*' | '>') { ' ' } else { c }).collect();
    let letters_joined = letters(&joined);
    let sound_only = cue && letters(&remainder) < 2;
    let note = lines.iter().any(|line| RE_NOTE.is_match(line));
    let note_loose = lines.iter().any(|line| RE_NOTE_LOOSE.is_match(line));
    let mut caps_lines = 0usize;
    let mut caps_cue = false;
    // for l, r in zip(lines, rest) (one rest line per line): the capital lines once cues and labels are out.
    for r in &rest {
        if letters(r) >= 4 && !r.chars().any(|c| c.is_ascii_lowercase()) {
            caps_lines += 1;
            if sound_named(r) {
                caps_cue = true;
            }
        }
    }
    let all_caps = letters(&joined) >= 3 && !joined.chars().any(|c| c.is_ascii_lowercase());
    let words: Vec<String> =
        RE_WORD.find_iter(&remainder.to_ascii_lowercase()).map(|word| word.as_str().to_string()).collect();
    Features {
        odd,
        note_only,
        tokens,
        wordlike: wordlike_tokens,
        cue,
        cue_sound,
        label,
        chev,
        letters: letters_joined,
        sound_only,
        note,
        note_loose,
        caps_lines,
        caps_cue,
        all_caps,
        lines: lines.len(),
        words,
    }
}

/// The language of a track's words by stopword shares (in STOP's order), or "short" under LANG_MIN_WORDS, or
/// "unknown"; another language needs a low English share and a high own share (the first of equal shares wins).
/// Py: lang_of.
fn lang_of(words: &[String], rule: &Rule) -> (String, Ordered<f64>) {
    let n = words.len();
    let mut shares = Ordered::new();
    for (lang, set) in STOP.iter() {
        let share = if n > 0 {
            words.iter().filter(|word| set.contains(word.as_str())).count() as f64 / n as f64
        } else {
            0.0
        };
        shares.insert(lang.to_string(), share);
    }
    if (n as i64) < rule.LANG_MIN_WORDS {
        return ("short".to_string(), shares);
    }
    let en = *shares.get("en").expect("STOP has en");
    let mut best: Option<(String, f64)> = None;
    for (lang, share) in shares.iter() {
        if lang == "en" {
            continue;
        }
        if best.as_ref().map_or(true, |(_, top)| *share > *top) {
            best = Some((lang.to_string(), *share));
        }
    }
    let (other, share) = best.expect("STOP has languages besides en");
    if en >= rule.EN_MIN {
        return ("en".to_string(), shares);
    }
    if en <= rule.EN_MAX_OTHER && share >= rule.OTHER_MIN && share >= rule.OTHER_RATIO * en {
        return (other, shares);
    }
    ("unknown".to_string(), shares)
}

/// Consecutive word pairs inside each event (dialogue words, at least 2 letters), leaving out pairs made of two
/// English stopwords, which any two English texts share. Py: bigrams.
fn bigrams(fs: &[Features]) -> BTreeSet<(String, String)> {
    let en = stop("en");
    let mut out = BTreeSet::new();
    for f in fs {
        let ws: Vec<&String> = f.words.iter().filter(|word| word.chars().count() >= 2).collect();
        for pair in ws.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if en.contains(a.as_str()) && en.contains(b.as_str()) {
                continue;
            }
            out.insert((a.clone(), b.clone()));
        }
    }
    out
}

/// A track's class. Serialised "sdh" | "regular" | "unclear" | "unreadable".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Class {
    Sdh,
    Regular,
    Unclear,
    Unreadable,
}

impl Class {
    /// The Python's word.
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Sdh => "sdh",
            Class::Regular => "regular",
            Class::Unclear => "unclear",
            Class::Unreadable => "unreadable",
        }
    }
}

impl std::str::FromStr for Class {
    type Err = String;
    fn from_str(text: &str) -> Result<Class, String> {
        match text {
            "sdh" => Ok(Class::Sdh),
            "regular" => Ok(Class::Regular),
            "unclear" => Ok(Class::Unclear),
            "unreadable" => Ok(Class::Unreadable),
            _ => Err(format!("not a subtitle class: {text}")),
        }
    }
}

/// The measures of one track: numbers only, every key of the Python dict, in its order (the serde form is subs.json's
/// "m"). Py: measure's dict.
#[allow(non_snake_case)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Measures {
    pub events: usize,
    pub text_events: usize,
    pub lines: usize,
    pub words: usize,
    pub note_only: usize,
    /// py_round(.., 3), 0.0 when no event is scored.
    pub letter_events: f64,
    pub wordlike: f64,
    pub readable: bool,
    /// "short", "en", "es", "fr", "de" or "unknown".
    pub lang: String,
    /// Stopword shares in STOP's order (en, es, fr, de), each py_round(.., 3).
    pub shares: Ordered<f64>,
    pub caps_share: f64,
    pub caps_style: bool,
    pub CUE: usize,
    pub LABEL: usize,
    pub CHEV: usize,
    pub M: usize,
    pub SO: usize,
    pub CS: usize,
    /// py_round(M / events, 4).
    pub rate: f64,
    pub CAPS_CUE: usize,
    pub NOTE: usize,
    pub NOTE_LOOSE: usize,
    pub ODD: usize,
    /// The correction counts (empty when no line was corrected: a stream not tagged English, or no line at all).
    pub corrections: Ordered<usize>,
    pub class: Class,
    /// The class's reason (numbers only).
    pub why: String,
}

/// What measure leaves on a track: the measures, the features per event, the word pairs (text: never written).
/// Py: track['m'], track['fs'], track['bigrams'].
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub m: Measures,
    pub fs: Vec<Features>,
    pub bigrams: BTreeSet<(String, String)>,
}

/// One subtitle stream as the rule reads it. Py: the track dicts of load_title and the tests.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub n: i64,
    /// The index's language, "und" when it has none.
    pub tag: String,
    pub codec: Option<String>,
    pub forced: bool,
    pub events: Vec<Event>,
    /// "text by <method>, <k> frames with text, <u> unmatched" when OCR text was paired (has_text).
    pub pairing: Option<String>,
    /// A text stream without OCR text (closed captions): set aside.
    pub set_aside: bool,
    /// Some(false): another language's stream without OCR text, used for timing only ('text': False).
    pub text: Option<bool>,
    /// Set by `measure`.
    pub measured: Option<Measured>,
}

impl Track {
    /// A track with no text facts yet: pairing None, set_aside false, text None, measured None.
    pub fn new(n: i64, tag: &str, codec: Option<&str>, forced: bool, events: Vec<Event>) -> Track {
        Track {
            n,
            tag: tag.to_string(),
            codec: codec.map(str::to_string),
            forced,
            events,
            pairing: None,
            set_aside: false,
            text: None,
            measured: None,
        }
    }

    /// Py: has_text ('pairing' in t).
    pub fn has_text(&self) -> bool {
        self.pairing.is_some()
    }
}

/// What measure left on `t`; a track the rule reads must be measured first (the Python's KeyError: 'm').
fn measured_of(t: &Track) -> &Measured {
    t.measured.as_ref().unwrap_or_else(|| panic!("stream s{} is not measured (the Python's KeyError: 'm')", t.n))
}

/// The measures of a measured track.
fn measures_of(t: &Track) -> &Measures {
    &measured_of(t).m
}

/// Measures a track (streams tagged 'eng' or 'en' get the corrections) and stores Measured on it. Py: measure.
pub fn measure<'t>(track: &'t mut Track, rule: &Rule) -> &'t Measures {
    let english = matches!(track.tag.as_str(), "eng" | "en");
    let mut counts: Ordered<usize> = Ordered::new();
    let fs: Vec<Features> =
        track.events.iter().map(|e| event_features(&e.lines, english, &mut counts, rule)).collect();
    let n = fs.len();
    let nlines: usize = fs.iter().map(|f| f.lines).sum();
    let toks: usize = fs.iter().map(|f| f.tokens).sum();
    let words: Vec<String> = fs.iter().flat_map(|f| f.words.iter().cloned()).collect();
    let scored: Vec<&Features> = fs.iter().filter(|f| !f.note_only).collect();
    let letter_events = if scored.is_empty() {
        0.0
    } else {
        py_round(scored.iter().filter(|f| f.letters >= 2).count() as f64 / scored.len() as f64, 3)
    };
    let wordlike = if toks > 0 {
        py_round(fs.iter().map(|f| f.wordlike).sum::<usize>() as f64 / toks as f64, 3)
    } else {
        0.0
    };
    let readable = letter_events >= rule.READ_LETTER_EVENTS
        && wordlike >= rule.READ_WORDLIKE
        && (words.len() as i64) >= rule.READ_MIN_WORDS;
    let (lang, shares) = lang_of(&words, rule);
    let shares = Ordered(shares.0.into_iter().map(|(lang, share)| (lang, py_round(share, 3))).collect());
    let caps_share = if nlines > 0 {
        py_round(fs.iter().map(|f| f.caps_lines).sum::<usize>() as f64 / nlines as f64, 3)
    } else {
        0.0
    };
    let caps_style = caps_share >= rule.CAPS_STYLE;
    let cs = caps_style;
    let markers = fs.iter().filter(|f| f.cue || f.chev || (f.label && !cs)).count();
    let mut m = Measures {
        events: n,
        text_events: fs.iter().filter(|f| f.letters > 0).count(),
        lines: nlines,
        words: words.len(),
        note_only: n - scored.len(),
        letter_events,
        wordlike,
        readable,
        lang,
        shares,
        caps_share,
        caps_style,
        CUE: fs.iter().filter(|f| f.cue).count(),
        LABEL: if cs { 0 } else { fs.iter().filter(|f| f.label).count() },
        CHEV: fs.iter().filter(|f| f.chev).count(),
        M: markers,
        SO: fs.iter().filter(|f| f.sound_only).count(),
        CS: fs.iter().filter(|f| f.cue_sound).count(),
        rate: if n > 0 { py_round(markers as f64 / n as f64, 4) } else { 0.0 },
        CAPS_CUE: if cs { 0 } else { fs.iter().filter(|f| f.caps_cue).count() },
        NOTE: fs.iter().filter(|f| f.note).count(),
        NOTE_LOOSE: fs.iter().filter(|f| f.note_loose).count(),
        ODD: fs.iter().filter(|f| f.odd).count(),
        corrections: counts,
        class: Class::Unreadable,
        why: String::new(),
    };
    let (class, why) = classify(&m, rule);
    m.class = class;
    m.why = why;
    let bigrams = bigrams(&fs);
    track.measured = Some(Measured { m, fs, bigrams });
    measures_of(track)
}

/// sdh, regular, unclear (kept, review) or unreadable (8.7 decides, review), and why. Py: classify.
pub fn classify(m: &Measures, rule: &Rule) -> (Class, String) {
    if !m.readable {
        let why = format!(
            "text not readable (letters {}, word-like {}, {} words)",
            py_fixed(m.letter_events, 2),
            py_fixed(m.wordlike, 2),
            m.words
        );
        return (Class::Unreadable, why);
    }
    if (m.M as i64) >= rule.SDH_M
        && m.rate >= rule.SDH_RATE
        && (m.SO as i64) >= rule.SDH_SO
        && (m.CS as i64) >= rule.SDH_CS
    {
        return (Class::Sdh, format!("cues and labels in {} events, {} sound-only", m.M, m.SO));
    }
    if (m.M as i64) <= rule.REG_M_MAX {
        if (m.ODD as i64) > rule.ODD_MAX {
            return (Class::Unclear, format!("{} events with damaged brackets", m.ODD));
        }
        if (m.CAPS_CUE as i64) > rule.CAPS_CUE_MAX {
            return (Class::Unclear, format!("{} capital sound lines without brackets (UK-style SDH?)", m.CAPS_CUE));
        }
        return (Class::Regular, "no SDH markers".to_string());
    }
    let why = format!("some SDH markers ({} events, {} sound-only, {} naming a sound)", m.M, m.SO, m.CS);
    (Class::Unclear, why)
}

/// Share of `a`'s events that start within `tol` of some event of `b` (spec 8.7), py_round(.., 3); 0.0 for no event.
/// Py: subtitle_text.aligned.
pub fn aligned(a: &[Event], b: &[Event], tol: f64) -> f64 {
    let starts = |events: &[Event]| -> Vec<f64> { events.iter().map(|e| e.start).collect() };
    py_round(align::aligned(&starts(a), &starts(b), tol), 3)
}

/// Share of `a`'s bigrams that also occur in `b`'s, anywhere in the title (time-free), py_round(.., 3); None when
/// `a` has fewer than CB_MIN_BIGRAMS. Both measured. Py: contained.
pub fn contained(a: &Track, b: &Track, rule: &Rule) -> Option<f64> {
    let (ab, bb) = (&measured_of(a).bigrams, &measured_of(b).bigrams);
    if (ab.len() as i64) < rule.CB_MIN_BIGRAMS {
        return None;
    }
    Some(py_round(ab.intersection(bb).count() as f64 / ab.len() as f64, 3))
}

/// `a`'s all-capital events that overlap no event of `b` padded by `tol` (the 50 spans starting before the event's
/// end are looked at, as the Python does). `a` measured. Py: uc.
pub fn uc(a: &Track, b: &Track, tol: f64) -> usize {
    let mut spans: Vec<(f64, f64)> = b.events.iter().map(|e| (e.start, e.start + py_max0(e.dur))).collect();
    spans.sort_by(|x, y| py_cmp(x.0, y.0).then(py_cmp(x.1, y.1)));
    let starts: Vec<f64> = spans.iter().map(|span| span.0).collect();
    let mut k = 0;
    for (e, f) in a.events.iter().zip(&measured_of(a).fs) {
        if !f.all_caps {
            continue;
        }
        let (lo, hi) = (e.start - tol, e.start + py_max0(e.dur) + tol);
        let i = bisect_right(&starts, hi);
        if !spans[i.saturating_sub(50)..i].iter().any(|&(s, t)| s <= hi && t >= lo) {
            k += 1;
        }
    }
    k
}

/// Timing of a pair: "same" | "diff" | "between".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Timing {
    Same,
    Diff,
    Between,
}

impl Timing {
    /// The Python's word.
    pub fn as_str(self) -> &'static str {
        match self {
            Timing::Same => "same",
            Timing::Diff => "diff",
            Timing::Between => "between",
        }
    }
}

impl std::str::FromStr for Timing {
    type Err = String;
    fn from_str(text: &str) -> Result<Timing, String> {
        match text {
            "same" => Ok(Timing::Same),
            "diff" => Ok(Timing::Diff),
            "between" => Ok(Timing::Between),
            _ => Err(format!("not a pair timing: {text}")),
        }
    }
}

/// Content of a pair: "same" | "different" | "partial" | "unknown".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Content {
    Same,
    Different,
    Partial,
    Unknown,
}

impl Content {
    /// The Python's word.
    pub fn as_str(self) -> &'static str {
        match self {
            Content::Same => "same",
            Content::Different => "different",
            Content::Partial => "partial",
            Content::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for Content {
    type Err = String;
    fn from_str(text: &str) -> Result<Content, String> {
        match text {
            "same" => Ok(Content::Same),
            "different" => Ok(Content::Different),
            "partial" => Ok(Content::Partial),
            "unknown" => Ok(Content::Unknown),
            _ => Err(format!("not a pair content: {text}")),
        }
    }
}

/// The pair measures of (a, b), each tuple as (a against b, b against a). Py: pair's dict.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Pair {
    pub aligned: (f64, f64),
    pub cb: (Option<f64>, Option<f64>),
    pub uc: (usize, usize),
    pub timing: Timing,
    pub content: Content,
}

impl Pair {
    /// The pair seen from the other track: dict(p, aligned=p['aligned'][::-1], cb=p['cb'][::-1], uc=p['uc'][::-1]).
    fn reversed(&self) -> Pair {
        Pair {
            aligned: (self.aligned.1, self.aligned.0),
            cb: (self.cb.1, self.cb.0),
            uc: (self.uc.1, self.uc.0),
            timing: self.timing,
            content: self.content,
        }
    }
}

/// Timing (the larger aligned share against SAME_MIN and DIFF_MAX) and content (bigram containment both ways,
/// unknown when a track is unreadable or too short). Both measured. Py: pair.
pub fn pair(a: &Track, b: &Track, rule: &Rule) -> Pair {
    let shares = (aligned(&a.events, &b.events, rule.TOL_S), aligned(&b.events, &a.events, rule.TOL_S));
    let cb = (contained(a, b, rule), contained(b, a, rule));
    let capitals = (uc(a, b, rule.TOL_S), uc(b, a, rule.TOL_S));
    let t = shares.0.max(shares.1);
    let timing = if t >= rule.SAME_MIN {
        Timing::Same
    } else if t <= rule.DIFF_MAX {
        Timing::Diff
    } else {
        Timing::Between
    };
    let content = match cb {
        (Some(x), Some(y)) if measures_of(a).readable && measures_of(b).readable => {
            if x.min(y) >= rule.CB_SAME {
                Content::Same
            } else if x.max(y) <= rule.CB_DIFF {
                Content::Different
            } else {
                Content::Partial
            }
        }
        _ => Content::Unknown,
    };
    Pair { aligned: shares, cb, uc: capitals, timing, content }
}

/// True when `a`'s words are a one-way subset of `b`'s: forced (or partial) content, whatever 8.7's 'full' says.
/// Never for an unreadable track: 8.7 decides those (T0). `cb` is (contained(a, b), contained(b, a)).
/// Py: subset_of (whose `p` the Python also passes as {'cb': reversed}).
pub fn subset_of(a: &Track, b: &Track, cb: (Option<f64>, Option<f64>), rule: &Rule) -> bool {
    if !(measures_of(a).readable && measures_of(b).readable) {
        return false;
    }
    let (Some(x), Some(y)) = cb else {
        return false;
    };
    x >= rule.CB_SUBSET_IN
        && y < rule.CB_SUBSET_BACK
        && (a.events.len() as f64) <= rule.SUBSET_EVENTS * (b.events.len() as f64)
}
// ------------------------------------------------------------------ the rule (section 3 of the design)

/// The review reason of every reviewed decision.
const AMBIGUOUS: &str = "subtitle_ambiguous";

/// A role of the rule. Serialised as the Python's strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role {
    #[serde(rename = "sdh")]
    Sdh,
    #[serde(rename = "regular")]
    Regular,
    #[serde(rename = "forced")]
    Forced,
    #[serde(rename = "drop")]
    Drop,
    #[serde(rename = "keep")]
    Keep,
    #[serde(rename = "set aside")]
    SetAside,
}

impl Role {
    /// The Python's word.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Sdh => "sdh",
            Role::Regular => "regular",
            Role::Forced => "forced",
            Role::Drop => "drop",
            Role::Keep => "keep",
            Role::SetAside => "set aside",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(text: &str) -> Result<Role, String> {
        match text {
            "sdh" => Ok(Role::Sdh),
            "regular" => Ok(Role::Regular),
            "forced" => Ok(Role::Forced),
            "drop" => Ok(Role::Drop),
            "keep" => Ok(Role::Keep),
            "set aside" => Ok(Role::SetAside),
            _ => Err(format!("not a subtitle role: {text}")),
        }
    }
}

/// One stream's decision: {'role', 'review', 'note'}; review is None or "subtitle_ambiguous". Py: decide's values.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Decision {
    pub role: Role,
    pub review: Option<String>,
    pub note: Option<String>,
}

/// out[n] = {'role': role, 'review': review, 'note': note} (a later put replaces an earlier one). Py: decide's put.
fn put(out: &mut BTreeMap<i64, Decision>, n: i64, role: Role, review: bool, note: Option<&str>) {
    out.insert(n, Decision { role, review: review.then(|| AMBIGUOUS.to_string()), note: note.map(str::to_string) });
}

/// bool() of a review or note: None and "" are false.
fn truthy(text: Option<&str>) -> bool {
    text.is_some_and(|text| !text.is_empty())
}

/// `orig_lang or 'unknown'`.
fn or_unknown(orig_lang: Option<&str>) -> &str {
    match orig_lang {
        Some(lang) if !lang.is_empty() => lang,
        _ => "unknown",
    }
}

/// The tags the rule reads as English. Py: ('eng', 'en').
fn is_english(tag: &str) -> bool {
    matches!(tag, "eng" | "en")
}

/// The tags that name no language (the index's empty language is "und" by then). Py: ('und', '', None).
fn is_untagged(tag: &str) -> bool {
    matches!(tag, "und" | "")
}

/// Role of every stream (section 3 of EXP-14-design.md, T0 to T7 as the Python implements them): every English or
/// no-language stream not set aside is measured first; another language's stream may be unread (`text:
/// Some(false)`) and then serves T4's timing only; a closed-caption text stream is set aside. Returns n -> decision;
/// a track with a review is never dropped (ambiguous = keep). Py: decide.
pub fn decide(tracks: &[Track], orig_lang: Option<&str>, rule: &Rule) -> BTreeMap<i64, Decision> {
    let mut out: BTreeMap<i64, Decision> = BTreeMap::new();
    let mut eng: Vec<usize> = Vec::new();
    for (i, t) in tracks.iter().enumerate() {
        if t.set_aside {
            put(&mut out, t.n, Role::SetAside, false, Some("closed captions, a text stream (8.7: set aside first)"));
            continue;
        }
        if !is_english(&t.tag) && !is_untagged(&t.tag) {
            let note = format!("not English (tag {})", t.tag);
            put(&mut out, t.n, Role::Drop, false, Some(note.as_str()));
            continue;
        }
        let m = measures_of(t);
        if is_english(&t.tag) {
            if m.readable && !matches!(m.lang.as_str(), "en" | "unknown" | "short") {
                let note = format!("tagged eng, text reads {}", m.lang);
                put(&mut out, t.n, Role::Keep, true, Some(note.as_str()));
                continue;
            }
            eng.push(i);
        } else if m.readable && m.lang == "en" {
            eng.push(i);
        } else if !m.readable || matches!(m.lang.as_str(), "short" | "unknown") {
            // No tag and no language in the text: it may be English, so it is never dropped (T1c).
            put(&mut out, t.n, Role::Keep, true, Some("no language tag and the text does not show its language"));
        } else {
            let note = format!("not English (tag {})", t.tag);
            put(&mut out, t.n, Role::Drop, false, Some(note.as_str()));
        }
    }
    if eng.is_empty() {
        return out;
    }
    let unclear_lang: BTreeSet<i64> = eng
        .iter()
        .filter(|&&i| measures_of(&tracks[i]).readable && measures_of(&tracks[i]).lang == "unknown")
        .map(|&i| tracks[i].n)
        .collect();
    let mut pairs: BTreeMap<(i64, i64), Pair> = BTreeMap::new();
    for (x, &a) in eng.iter().enumerate() {
        for &b in &eng[x + 1..] {
            let p = pair(&tracks[a], &tracks[b], rule);
            let reversed = p.reversed();
            pairs.insert((tracks[a].n, tracks[b].n), p);
            pairs.insert((tracks[b].n, tracks[a].n), reversed);
        }
    }
    let largest = eng.iter().map(|&i| tracks[i].events.len()).max().expect("eng is not empty");
    // rank: T7's order of the forced candidates (8.7's flagged ones, then 8.7's not full with < 150 events, then T2's).
    let (mut full, mut cands, mut partial) = (Vec::new(), Vec::new(), Vec::new());
    let mut rank: BTreeMap<i64, u8> = BTreeMap::new();
    for &i in &eng {
        let t = &tracks[i];
        if t.forced {
            cands.push(i);
            rank.insert(t.n, 0);
        } else if (t.events.len() as f64) >= rule.FULL_SHARE * (largest as f64) {
            full.push(i);
        } else if (t.events.len() as i64) < rule.FORCED_MAX_EVENTS {
            cands.push(i);
            rank.insert(t.n, 1);
        } else {
            partial.push(i);
        }
    }
    // A full track whose words are a one-way subset of a larger full track is a forced candidate. Every track is
    // compared with the full tracks as they were before this step, so the stream numbers never change the verdict:
    // in a chain (A in B, B in C) A is a candidate although B is one too.
    let before = full.clone();
    for &t in &before {
        let tn = tracks[t].n;
        if before.iter().any(|&u| u != t && subset_of(&tracks[t], &tracks[u], pairs[&(tn, tracks[u].n)].cb, rule)) {
            full.retain(|&x| x != t);
            cands.push(t);
            rank.insert(tn, 2);
        }
    }
    // A not-full track with 150 events or more (8.7: partial_subtitle_dropped) is a forced candidate first when its
    // words are a one-way subset of a full track; when its text cannot be read, or its language is unclear, it is
    // kept for review (T0, T1b, T7).
    for &t in &partial {
        let tn = tracks[t].n;
        if full.iter().any(|&u| subset_of(&tracks[t], &tracks[u], pairs[&(tn, tracks[u].n)].cb, rule)) {
            cands.push(t);
            rank.insert(tn, 2);
        } else if !measures_of(&tracks[t]).readable {
            put(&mut out, tn, Role::Keep, true, Some("subtitle_text_unreadable (8.7 would drop it as partial)"));
        } else if unclear_lang.contains(&tn) {
            put(&mut out, tn, Role::Keep, true, Some("language of the text unclear (8.7 would drop it as partial)"));
        } else {
            put(&mut out, tn, Role::Drop, false, Some("partial_subtitle_dropped"));
        }
    }
    // Groups: same words join (whatever the timing); timing joins when the text cannot be read.
    let mut group: BTreeMap<i64, BTreeSet<i64>> =
        full.iter().map(|&i| (tracks[i].n, BTreeSet::from([tracks[i].n]))).collect();
    let mut unsure: BTreeSet<i64> = BTreeSet::new();
    for (x, &a) in full.iter().enumerate() {
        for &b in &full[x + 1..] {
            let (an, bn) = (tracks[a].n, tracks[b].n);
            let p = &pairs[&(an, bn)];
            let (c, tm) = (p.content, p.timing);
            let join = c == Content::Same || (c == Content::Unknown && tm == Timing::Same);
            if !join
                && !(c == Content::Different && tm == Timing::Diff)
                && !(c == Content::Unknown && tm == Timing::Diff)
            {
                unsure.insert(an);
                unsure.insert(bn);
            }
            if join {
                let g: BTreeSet<i64> = group[&an].union(&group[&bn]).copied().collect();
                for k in &g {
                    group.insert(*k, g.clone());
                }
            }
        }
    }
    let mut groups: Vec<BTreeSet<i64>> = Vec::new();
    for &t in &full {
        let g = &group[&tracks[t].n];
        if !groups.contains(g) {
            groups.push(g.clone());
        }
    }
    let by_n: BTreeMap<i64, usize> = eng.iter().map(|&i| (tracks[i].n, i)).collect();
    let mut dialogue: Option<BTreeSet<i64>> = None;
    if groups.len() == 1 {
        dialogue = Some(groups[0].clone());
    } else if !groups.is_empty() {
        // sorted(groups, key=len, reverse=True): stable, the larger first.
        let mut big = groups.clone();
        big.sort_by(|x, y| y.len().cmp(&x.len()));
        if big[0].len() >= 2 && big[1].len() < big[0].len() {
            dialogue = Some(big[0].clone());
        } else if big[0].len() == 1 {
            // Only single-track groups: the one whose track follows a full subtitle track of another language (that
            // track's timing is the dialogue's), when exactly one does. Timing, not SDH markers, decides. Another
            // language's stream counts here whether its text was read or not (only its packets are used).
            let others: Vec<usize> = (0..tracks.len())
                .filter(|&i| {
                    let t = &tracks[i];
                    !eng.contains(&i)
                        && !is_untagged(&t.tag)
                        && !t.set_aside
                        && (t.events.len() as f64) >= rule.FULL_SHARE * (largest as f64)
                })
                .collect();
            let hits: Vec<&BTreeSet<i64>> = groups
                .iter()
                .filter(|g| {
                    g.iter().any(|k| {
                        others.iter().any(|&o| {
                            let (tk, to) = (&tracks[by_n[k]], &tracks[o]);
                            let s = aligned(&tk.events, &to.events, rule.TOL_S)
                                .max(aligned(&to.events, &tk.events, rule.TOL_S));
                            s >= rule.SAME_MIN
                        })
                    })
                })
                .collect();
            if hits.len() == 1 {
                dialogue = Some(hits[0].clone());
            }
        }
    }
    match &dialogue {
        None => {
            for &t in &full {
                put(&mut out, tracks[t].n, Role::Keep, true, Some("no dialogue group"));
            }
        }
        Some(dlg) => {
            let members: Vec<usize> = dlg.iter().map(|k| by_n[k]).collect();
            roles(tracks, &members, &pairs, &mut out, rule);
            for &t in &full {
                let tn = tracks[t].n;
                if dlg.contains(&tn) {
                    continue;
                }
                let m = measures_of(&tracks[t]);
                let vs: Vec<&Pair> = dlg.iter().map(|k| &pairs[&(tn, *k)]).collect();
                let other_words = vs.iter().all(|p| p.content == Content::Different);
                let other_time = vs.iter().all(|p| p.timing == Timing::Diff);
                if unsure.contains(&tn) {
                    put(&mut out, tn, Role::Keep, true, Some("text and timing disagree with the dialogue group"));
                } else if !m.readable {
                    put(&mut out, tn, Role::Keep, true, Some("subtitle_text_unreadable (8.7 would drop it by timing)"));
                } else if other_words
                    && other_time
                    && m.SO == 0
                    && m.class != Class::Sdh
                    && group[&tn].len() == 1
                    && m.lang == "en"
                    && orig_lang == Some("en")
                {
                    put(&mut out, tn, Role::Drop, false, Some("commentary or trivia: other words and other timing"));
                } else {
                    let mut why = "outside the dialogue group".to_string();
                    if orig_lang != Some("en") {
                        why = format!("original language {}", or_unknown(orig_lang));
                    }
                    if unclear_lang.contains(&tn) {
                        why = format!("language of the text unclear; {why}");
                    }
                    put(&mut out, tn, Role::Keep, true, Some(why.as_str()));
                }
            }
        }
    }
    // Forced candidates in T7's order, each confirmed by its words. A candidate whose text reads another language
    // never gets here: T1a keeps it for review.
    let reference: Option<usize> = dialogue.as_ref().map(|dlg| {
        let with_role = |role: Role| -> Vec<usize> {
            dlg.iter().filter(|k| out.get(*k).map(|d| d.role) == Some(role)).map(|k| by_n[k]).collect()
        };
        let (regs, sdhs) = (with_role(Role::Regular), with_role(Role::Sdh));
        let all: Vec<usize> = dlg.iter().map(|k| by_n[k]).collect();
        regs.first().or(sdhs.first()).or(all.first()).copied().expect("a dialogue group has a member")
    });
    let mut kept = false;
    let mut order = cands.clone();
    order.sort_by_key(|&i| (rank[&tracks[i].n], tracks[i].n));
    for t in order {
        let tr = &tracks[t];
        let m = measures_of(tr);
        let x = match reference {
            Some(r) if m.readable && measures_of(&tracks[r]).readable => contained(tr, &tracks[r], rule),
            _ => None,
        };
        let lyrics = m.events > 0 && (m.NOTE as f64) / (m.events as f64) >= rule.NOTE_LINES;
        if lyrics {
            let note = format!("forced candidate looks like lyrics ({} note lines)", m.NOTE);
            put(&mut out, tr.n, Role::Keep, true, Some(note.as_str()));
        } else if x.is_some_and(|x| x <= rule.CB_DIFF) {
            put(&mut out, tr.n, Role::Keep, true, Some("forced candidate shares no words with the dialogue track"));
        } else if x.is_some_and(|x| x < rule.FORCED_CB) {
            put(&mut out, tr.n, Role::Keep, true, Some("forced candidate partly matches the dialogue track"));
        } else if kept {
            put(&mut out, tr.n, Role::Keep, true, Some("a second forced candidate"));
        } else if !m.readable {
            put(&mut out, tr.n, Role::Forced, true, Some("subtitle_text_unreadable"));
            kept = true;
        } else if reference.is_some_and(|r| !measures_of(&tracks[r]).readable) {
            put(&mut out, tr.n, Role::Forced, true, Some("dialogue track unreadable"));
            kept = true;
        } else {
            let note = if x.is_some() {
                "words found in the dialogue track"
            } else if reference.is_none() {
                "no dialogue track to compare with"
            } else {
                "too short to compare"
            };
            put(&mut out, tr.n, Role::Forced, false, Some(note));
            kept = true;
        }
    }
    for k in &unclear_lang {
        let Some(v) = out.get(k).cloned() else {
            continue;
        };
        if v.role == Role::Drop {
            let note = format!(
                "language of the text unclear (the rule would drop it: {})",
                v.note.as_deref().unwrap_or("None")
            );
            put(&mut out, *k, Role::Keep, true, Some(note.as_str()));
        } else if !truthy(v.review.as_deref()) {
            put(&mut out, *k, v.role, true, Some("language of the text unclear"));
        }
    }
    // Ambiguous = keep: a track held for review is never dropped, whatever step reviewed it. Unreachable today, like
    // the drop branch just above: a guard so that no later step can bring back a reviewed drop.
    for v in out.values_mut() {
        if v.role == Role::Drop && truthy(v.review.as_deref()) {
            v.role = Role::Keep;
        }
    }
    out
}

/// Regular and SDH inside the dialogue group (replaces 8.7's 'the one with more events is SDH'); `members` are the
/// group's tracks in stream-number order. Py: roles.
fn roles(
    tracks: &[Track],
    members: &[usize],
    pairs: &BTreeMap<(i64, i64), Pair>,
    out: &mut BTreeMap<i64, Decision>,
    rule: &Rule,
) {
    if members.len() == 1 {
        let t = &tracks[members[0]];
        let m = measures_of(t);
        match m.class {
            Class::Sdh => put(out, t.n, Role::Sdh, false, None),
            Class::Regular => put(out, t.n, Role::Regular, false, None),
            Class::Unreadable => put(out, t.n, Role::Regular, true, Some("subtitle_text_unreadable")),
            Class::Unclear => put(out, t.n, Role::Regular, true, Some(m.why.as_str())),
        }
        return;
    }
    if members.len() > 2 {
        for &i in members {
            put(out, tracks[i].n, Role::Keep, true, Some("more than two tracks in the dialogue group"));
        }
        return;
    }
    let (a, b) = (&tracks[members[0]], &tracks[members[1]]);
    let (ca, cb) = (measures_of(a).class, measures_of(b).class);
    if ca == Class::Unreadable || cb == Class::Unreadable {
        let (hi, lo) = if a.events.len() >= b.events.len() { (a, b) } else { (b, a) };
        put(out, hi.n, Role::Sdh, true, Some("subtitle_text_unreadable: SDH by event count (8.7)"));
        put(out, lo.n, Role::Regular, true, Some("subtitle_text_unreadable"));
    } else if (ca == Class::Sdh && cb == Class::Regular) || (ca == Class::Regular && cb == Class::Sdh) {
        let (s, r) = if ca == Class::Sdh { (a, b) } else { (b, a) };
        let note = if s.events.len() >= r.events.len() { None } else { Some("subtitle_count_disagrees") };
        put(out, s.n, Role::Sdh, false, note);
        put(out, r.n, Role::Regular, false, note);
    } else if ca == Class::Regular && cb == Class::Regular {
        let p = &pairs[&(a.n, b.n)];
        let (u0, u1) = p.uc;
        let why = if (u0.max(u1) as i64) >= rule.UC_MIN && u0.min(u1) <= 1 {
            "capital-only events in one track only (UK-style SDH?)"
        } else {
            "two regular tracks"
        };
        put(out, a.n, Role::Regular, true, Some(why));
        put(out, b.n, Role::Regular, true, Some(why));
    } else if ca == Class::Sdh && cb == Class::Sdh {
        put(out, a.n, Role::Sdh, true, Some("two SDH tracks"));
        put(out, b.n, Role::Sdh, true, Some("two SDH tracks"));
    } else {
        // One or both unclear: kept for review. When the other one is SDH, that is the suggestion shown.
        let why = [a, b]
            .iter()
            .filter(|t| measures_of(t).class == Class::Unclear)
            .map(|t| measures_of(t).why.as_str())
            .collect::<Vec<&str>>()
            .join("; ");
        for (t, u) in [(a, b), (b, a)] {
            let role = if measures_of(t).class == Class::Sdh {
                Role::Sdh
            } else if measures_of(u).class == Class::Sdh {
                Role::Regular
            } else {
                Role::Keep
            };
            put(out, t.n, role, true, Some(why.as_str()));
        }
    }
}

/// What spec 8.7 does today, from event counts and start times alone (for comparison): no TheDiscDB and no
/// forced-only parent. A role with " (review)" raised subtitle_ambiguous. The dialogue group is a `PySet`, iterated
/// in the Python's order. Py: spec87.
pub fn spec87(tracks: &[Track], rule: &Rule) -> BTreeMap<i64, String> {
    let mut out: BTreeMap<i64, String> = BTreeMap::new();
    let eng: Vec<usize> = (0..tracks.len()).filter(|&i| is_english(&tracks[i].tag) && !tracks[i].set_aside).collect();
    for (i, t) in tracks.iter().enumerate() {
        if t.set_aside {
            out.insert(t.n, "set aside".to_string());
        } else if !eng.contains(&i) {
            out.insert(t.n, "drop".to_string());
        }
    }
    if eng.is_empty() {
        return out;
    }
    let largest = eng.iter().map(|&i| tracks[i].events.len()).max().expect("eng is not empty");
    let full: Vec<usize> = eng
        .iter()
        .copied()
        .filter(|&i| !tracks[i].forced && (tracks[i].events.len() as f64) >= rule.FULL_SHARE * (largest as f64))
        .collect();
    let rest: Vec<usize> = eng.iter().copied().filter(|i| !full.contains(i)).collect();
    // group[n] is an index into `sets`: Python's dict of set objects, whose identity matters (a | a is a copy).
    let mut sets: Vec<PySet> = Vec::new();
    let mut group: BTreeMap<i64, usize> = BTreeMap::new();
    for &i in &full {
        sets.push(PySet::singleton(tracks[i].n));
        group.insert(tracks[i].n, sets.len() - 1);
    }
    let mut review = false;
    for (x, &a) in full.iter().enumerate() {
        for &b in &full[x + 1..] {
            let (ta, tb) = (&tracks[a], &tracks[b]);
            let s = aligned(&ta.events, &tb.events, rule.TOL_S).max(aligned(&tb.events, &ta.events, rule.TOL_S));
            if s >= rule.SAME_MIN {
                let (ga, gb) = (group[&ta.n], group[&tb.n]);
                let g = if ga == gb { sets[ga].copy() } else { sets[ga].union(&sets[gb]) };
                sets.push(g);
                let id = sets.len() - 1;
                let members: Vec<i64> = sets[id].iter().collect();
                for k in members {
                    group.insert(k, id);
                }
            } else if s > rule.DIFF_MAX {
                review = true;
            }
        }
    }
    // Every member of a group points at the group's one set, so equal groups are the same index.
    let mut groups: Vec<usize> = Vec::new();
    for &t in &full {
        let id = group[&tracks[t].n];
        if !groups.contains(&id) {
            groups.push(id);
        }
    }
    let mut dialogue: Option<usize> = None;
    if groups.len() == 1 {
        dialogue = Some(groups[0]);
    } else if !groups.is_empty() {
        let mut big = groups.clone();
        big.sort_by(|x, y| sets[*y].len().cmp(&sets[*x].len()));
        if sets[big[0]].len() >= 2 && sets[big[1]].len() < sets[big[0]].len() {
            dialogue = Some(big[0]);
        }
    }
    let tag = if review { " (review)" } else { "" };
    let by_n: BTreeMap<i64, usize> = full.iter().map(|&i| (tracks[i].n, i)).collect();
    match dialogue {
        None => {
            for &t in &full {
                out.insert(tracks[t].n, "keep (review)".to_string());
            }
        }
        Some(id) => {
            // sorted((by_n[k] for k in dialogue), key=lambda t: -len(t['events'])): set order, then stable.
            let mut ms: Vec<usize> = sets[id].iter().map(|k| by_n[&k]).collect();
            ms.sort_by(|x, y| tracks[*y].events.len().cmp(&tracks[*x].events.len()));
            match ms.len() {
                1 => {
                    out.insert(tracks[ms[0]].n, format!("regular{tag}"));
                }
                2 => {
                    out.insert(tracks[ms[0]].n, format!("sdh{tag}"));
                    out.insert(tracks[ms[1]].n, format!("regular{tag}"));
                }
                _ => {
                    for &t in &ms {
                        out.insert(tracks[t].n, "keep (review)".to_string());
                    }
                }
            }
            for &t in &full {
                if !sets[id].contains(tracks[t].n) {
                    out.insert(tracks[t].n, format!("drop{tag}"));
                }
            }
        }
    }
    let mut kept = false;
    let mut order = rest.clone();
    order.sort_by_key(|&i| (!tracks[i].forced, tracks[i].n));
    for t in order {
        let tr = &tracks[t];
        if tr.forced || (tr.events.len() as i64) < rule.FORCED_MAX_EVENTS {
            out.insert(tr.n, (if kept { "drop" } else { "forced" }).to_string());
            kept = true;
        } else {
            out.insert(tr.n, "drop".to_string());
        }
    }
    out
}

/// The outcome of `role` (with a review or not) until experiment 14 answers yes: 8.7's role (today), and
/// subtitle_ambiguous with the note subtitle_text_disagrees when the rule differs: another role, or a review that
/// 8.7 does not raise. Takes the role as text so that analyse's '?' for a stream without a decision fits.
fn before_yes_text(role: &str, reviewed: bool, today: &str) -> String {
    let today_role = today.replace(" (review)", "");
    let today_reviewed = today.ends_with(" (review)");
    if role != today_role || (reviewed && !today_reviewed) {
        return format!("{today_role} (review: subtitle_text_disagrees)");
    }
    today.to_string()
}

/// The outcome until experiment 14 answers yes: `today`, or "<role> (review: subtitle_text_disagrees)" when the rule
/// differs. Py: before_yes.
pub fn before_yes(v: &Decision, today: &str) -> String {
    before_yes_text(v.role.as_str(), truthy(v.review.as_deref()), today)
}
// ------------------------------------------------------------------ printing and the title

use sha1::{Digest, Sha1};

use crate::recording::files::{read_jsonl, FrameEntry, PacketsFile, SubsIndexEntry, Tmdb};
use crate::recording::{read_json, read_text, Encoding, Errors, PathKey, Tree};

/// The table analyse prints, header and rule. Py: analyse's two literal lines.
const TABLE_HEADER: &str = "| Stream | Tag | Events | Class | Rule | Review | Note | 8.7 today | Before yes |";
const TABLE_RULE: &str = "|---|---|---|---|---|---|---|---|---|";

/// The per-stream measures line (numbers only). Py: track_line.
pub fn track_line(t: &Track) -> String {
    let m = measures_of(t);
    let shares: Vec<String> = m.shares.iter().map(|(lang, share)| format!("{lang}={}", py_fixed(*share, 3))).collect();
    format!(
        "stream s{} {}: events={} lines={} words={} letters={} wordlike={} readable={} lang={} ({}) CUE={} LABEL={} \
         CHEV={} M={} rate={} SO={} CS={} caps_share={} CAPS_CUE={} NOTE={} NOTE_LOOSE={} ODD={} class={}",
        t.n,
        t.tag,
        m.events,
        m.lines,
        m.words,
        py_fixed(m.letter_events, 3),
        py_fixed(m.wordlike, 3),
        if m.readable { "yes" } else { "no" },
        m.lang,
        shares.join(" "),
        m.CUE,
        m.LABEL,
        m.CHEV,
        m.M,
        py_fixed(m.rate, 4),
        m.SO,
        m.CS,
        py_fixed(m.caps_share, 3),
        m.CAPS_CUE,
        m.NOTE,
        m.NOTE_LOOSE,
        m.ODD,
        m.class.as_str()
    )
}

/// 'n/a' or '%.3f'. Py: fmt.
fn fmt3(x: Option<f64>) -> String {
    match x {
        None => "n/a".to_string(),
        Some(x) => py_fixed(x, 3),
    }
}

/// `t.get('codec') or '?'`.
fn codec_or_question(t: &Track) -> &str {
    match t.codec.as_deref() {
        Some(codec) if !codec.is_empty() => codec,
        _ => "?",
    }
}

/// A text codec name (the DVD closed captions, which MakeMKV converts to SubRip): one of the words of mkvmerge's
/// text subtitle codec names. Py: is_text_codec.
pub fn is_text_codec(codec: Option<&str>) -> bool {
    align::is_text(codec)
}

/// One printed pair: its numbers and the subset phrase ("s0 in s1" or "no").
#[derive(Clone, Debug, PartialEq)]
pub struct PairRow {
    pub a: i64,
    pub b: i64,
    pub pair: Pair,
    pub subset: String,
}

/// What analyse printed and decided.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub lines: Vec<String>,
    pub decide: BTreeMap<i64, Decision>,
    pub spec87: BTreeMap<i64, String>,
    pub pairs: Vec<PairRow>,
}

/// The table cells of one stream: (rule role, review, note, 8.7 today, before yes), with the Python's '?' and '-'
/// for what is missing (a stream decide or spec87 left out; none does today).
fn table_cells<'a>(
    verdict: Option<&'a Decision>,
    today: Option<&'a String>,
) -> (&'a str, &'a str, &'a str, &'a str, String) {
    let (role, review, note) = match verdict {
        Some(v) => (v.role.as_str(), v.review.as_deref(), v.note.as_deref()),
        None => ("?", None, None),
    };
    let today = today.map_or("?", String::as_str);
    let dash = |text: Option<&'a str>| -> &'a str {
        match text {
            Some(text) if !text.is_empty() => text,
            _ => "-",
        }
    };
    (role, dash(review), dash(note), today, before_yes_text(role, truthy(review), today))
}

/// Measures every read track and prints the stream, pair, table and row: lines, then decides; `label` is the rip
/// label (the row lines' "<label>/evidence.mks"). Py: analyse.
pub fn analyse(tracks: &mut [Track], orig_lang: Option<&str>, label: &str, rule: &Rule) -> Analysis {
    let mut lines: Vec<String> = Vec::new();
    for t in tracks.iter_mut() {
        if t.set_aside {
            lines.push(format!(
                "stream s{} {}: set aside (closed captions: {} is a text stream; 8.7 sets it aside), {} events",
                t.n,
                t.tag,
                codec_or_question(t),
                t.events.len()
            ));
            continue;
        }
        if !t.has_text() {
            lines.push(format!(
                "stream s{} {}: not read (no OCR text), {} events, used for timing only",
                t.n,
                t.tag,
                t.events.len()
            ));
            continue;
        }
        measure(t, rule);
        lines.push(format!("{}  [{}]", track_line(t), t.pairing.as_deref().unwrap_or("")));
        let m = measures_of(t);
        // {k: v for k, v in sorted(corrections.items()) if v}
        let mut corrections: Vec<(&str, usize)> =
            m.corrections.iter().filter(|(_, count)| **count != 0).map(|(name, count)| (name, *count)).collect();
        corrections.sort();
        if !corrections.is_empty() {
            let shown: Vec<String> = corrections.iter().map(|(name, count)| format!("{name}={count}")).collect();
            lines.push(format!("  corrections: {}", shown.join(" ")));
        }
        if m.note_only > 0 {
            lines.push(format!("  note-only events: {} (left out of letters)", m.note_only));
        }
    }
    let tracks: &[Track] = tracks;
    let read: Vec<usize> = (0..tracks.len()).filter(|&i| tracks[i].has_text()).collect();
    let mut pairs: Vec<PairRow> = Vec::new();
    for (x, &a) in read.iter().enumerate() {
        for &b in &read[x + 1..] {
            let (ta, tb) = (&tracks[a], &tracks[b]);
            let p = pair(ta, tb, rule);
            let subset = if subset_of(ta, tb, p.cb, rule) {
                format!("s{} in s{}", ta.n, tb.n)
            } else if subset_of(tb, ta, (p.cb.1, p.cb.0), rule) {
                format!("s{} in s{}", tb.n, ta.n)
            } else {
                "no".to_string()
            };
            lines.push(format!(
                "pair s{} s{}: aligned={}/{} timing={} CB={}/{} content={} UC={}/{} subset={}",
                ta.n,
                tb.n,
                py_fixed(p.aligned.0, 3),
                py_fixed(p.aligned.1, 3),
                p.timing.as_str(),
                fmt3(p.cb.0),
                fmt3(p.cb.1),
                p.content.as_str(),
                p.uc.0,
                p.uc.1,
                subset
            ));
            pairs.push(PairRow { a: ta.n, b: tb.n, pair: p, subset });
        }
    }
    let verdict = decide(tracks, orig_lang, rule);
    let today = spec87(tracks, rule);
    lines.push(format!("original language: {}", or_unknown(orig_lang)));
    lines.push(TABLE_HEADER.to_string());
    lines.push(TABLE_RULE.to_string());
    for t in tracks {
        let class = if t.has_text() {
            measures_of(t).class.as_str()
        } else if t.set_aside {
            "set aside"
        } else {
            "not read"
        };
        let (role, review, note, today_role, before) = table_cells(verdict.get(&t.n), today.get(&t.n));
        lines.push(format!(
            "| s{} | {} | {} | {} | {} | {} | {} | {} | {} |",
            t.n,
            t.tag,
            t.events.len(),
            class,
            role,
            review,
            note,
            today_role,
            before
        ));
    }
    for t in tracks {
        let v = verdict.get(&t.n);
        let what = if t.has_text() {
            let m = measures_of(t);
            format!("M {}, SO {}, CS {}, lang {}, {}", m.M, m.SO, m.CS, m.lang, m.class.as_str())
        } else if t.set_aside {
            "set aside (closed captions)".to_string()
        } else {
            "not read (timing only)".to_string()
        };
        // v.get('role') is None (printed "None") for a stream without a decision; the review part shows the note.
        let role = v.map_or("None", |v| v.role.as_str());
        let review = match v {
            Some(v) if truthy(v.review.as_deref()) => format!(" (review: {})", v.note.as_deref().unwrap_or("None")),
            _ => String::new(),
        };
        lines.push(format!(
            "row: | {}/evidence.mks | s{}: {} {}, {} events | {}: {}{} | |",
            label,
            t.n,
            codec_or_question(t),
            t.tag,
            t.events.len(),
            what,
            role,
            review
        ));
    }
    Analysis { lines, decide: verdict, spec87: today, pairs }
}

/// frame -> line count of sub-<n>.frames.json when the file exists (a later entry for a frame wins). Py: the counts
/// of ocr_records.
fn frame_counts(tree: &dyn Tree, path: &PathKey) -> Result<BTreeMap<i64, Option<i64>>, String> {
    let mut counts = BTreeMap::new();
    if tree.exists(path) {
        let frames: Vec<FrameEntry> = read_json(tree, path)?;
        for f in frames {
            counts.insert(f.frame, f.lines);
        }
    }
    Ok(counts)
}

/// Every stream of `<rec>/subs/index.json` with its displayed events (a text stream: every packet, spec 8.7). A
/// stream with OCR text (`<text>/sub-<n>.ocr.jsonl` in `text_tree`, paired with `<rec>/subs-ocr/sub-<n>.frames.json`)
/// gets its lines and its pairing line; a text stream without OCR text is set aside (closed captions, 8.7); any other
/// stream without OCR text is marked unread (`text: Some(false)`). Err is the text the Python's "cannot read the
/// input" line printed (the FileNotFoundError, int(), float() or decoding text; serde_json's text for a JSON
/// error). Py: load_title.
pub fn load_title(
    rec_tree: &dyn Tree,
    rec: &PathKey,
    text_tree: &dyn Tree,
    text: &PathKey,
    clear_max_bytes: i64,
) -> Result<Vec<Track>, String> {
    let index: Vec<SubsIndexEntry> = read_json(rec_tree, &rec.join("subs/index.json"))?;
    let mut tracks = Vec::new();
    for entry in &index {
        let n = entry.n;
        let codec = entry.codec.as_deref();
        let txt = is_text_codec(codec);
        let packets: PacketsFile = read_json(rec_tree, &rec.join(&format!("subs/sub-{n}.packets.json")))?;
        let mut events = packet_events(packets.packets(), if txt { 0 } else { clear_max_bytes })?;
        let tag = match entry.language.as_deref() {
            Some(language) if !language.is_empty() => language,
            _ => "und",
        };
        let mut t = Track::new(n, tag, codec, entry.forced.unwrap_or(false), Vec::new());
        let jsonl = text.join(&format!("sub-{n}.ocr.jsonl"));
        if text_tree.exists(&jsonl) {
            let records = read_jsonl(&read_text(text_tree, &jsonl, Encoding::Utf8Sig, Errors::Strict)?)?;
            let counts = frame_counts(rec_tree, &rec.join(&format!("subs-ocr/sub-{n}.frames.json")))?;
            let recs = ocr_records(&records, &counts)?;
            let (method, left) = pair_text(&mut events, &recs);
            t.pairing = Some(format!("text by {}, {} frames with text, {} unmatched", method.as_str(), recs.len(), left));
        } else if txt {
            t.set_aside = true;
        } else {
            t.text = Some(false);
        }
        t.events = events;
        tracks.push(t);
    }
    Ok(tracks)
}

/// One `subs` run: stdout and stderr lines, the exit code (0; 2 for unreadable input, no OCR text at all, or an
/// English or no-language bitmap stream without it), the tracks, the original language used, the analysis (None
/// on exit 2). Py: subtitle_text.main.
#[derive(Clone, Debug, PartialEq)]
pub struct SubsRun {
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub exit_code: i32,
    pub tracks: Vec<Track>,
    pub orig_lang: Option<String>,
    pub analysis: Option<Analysis>,
}

/// main's flow after argument parsing: load_title, the two input checks, `orig_lang` or else
/// `<rec>/../tmdb.json`'s original_language, the header line, analyse. The folders are printed as the trees display
/// them (`str(Path)`). A tmdb.json that cannot be read crashed the Python; here it is "cannot read the input: ..."
/// and exit 2 (port spec 3.4). Py: subtitle_text.main.
pub fn subs_run(
    rec_tree: &dyn Tree,
    rec: &PathKey,
    text_tree: &dyn Tree,
    text: &PathKey,
    orig_lang: Option<&str>,
    clear_max_bytes: i64,
    rule: &Rule,
) -> SubsRun {
    let mut run =
        SubsRun { stdout: Vec::new(), stderr: Vec::new(), exit_code: 2, tracks: Vec::new(), orig_lang: None, analysis: None };
    match load_title(rec_tree, rec, text_tree, text, clear_max_bytes) {
        Ok(tracks) => run.tracks = tracks,
        Err(error) => {
            run.stderr.push(format!("cannot read the input: {error}"));
            return run;
        }
    }
    let rec_shown = rec_tree.display(rec);
    let text_shown = text_tree.display(text);
    if !run.tracks.iter().any(Track::has_text) {
        run.stderr.push(format!("no sub-<n>.ocr.jsonl in {text_shown} for the streams of {rec_shown}/subs/index.json"));
        return run;
    }
    // An English or no-language bitmap stream without text would leave the rule judging fewer tracks than the title
    // has: an input error, never a silent skip.
    let missing: Vec<(i64, String)> = run
        .tracks
        .iter()
        .filter(|t| t.text == Some(false) && matches!(t.tag.as_str(), "eng" | "en" | "und"))
        .map(|t| (t.n, t.tag.clone()))
        .collect();
    for (n, tag) in &missing {
        run.stderr.push(format!(
            "stream s{n} {tag}: no OCR text (no sub-{n}.ocr.jsonl in {text_shown}); read it with ocr-subs.ps1"
        ));
    }
    if !missing.is_empty() {
        return run;
    }
    let mut orig = orig_lang.map(str::to_string);
    let tmdb = rec.parent().join("tmdb.json");
    if orig.is_none() && rec_tree.exists(&tmdb) {
        match read_json::<Tmdb>(rec_tree, &tmdb) {
            Ok(info) => orig = info.original_language,
            Err(error) => {
                run.stderr.push(format!("cannot read the input: {error}"));
                return run;
            }
        }
    }
    let unread = run.tracks.iter().filter(|t| t.text == Some(false)).count();
    let aside = run.tracks.iter().filter(|t| t.set_aside).count();
    let mut extra = String::new();
    if unread > 0 {
        extra.push_str(&format!(", {unread} not read (another language)"));
    }
    if aside > 0 {
        extra.push_str(&format!(", {aside} set aside (closed captions)"));
    }
    let with_text = run.tracks.iter().filter(|t| t.has_text()).count();
    run.stdout.push(format!("{rec_shown}: {with_text} subtitle streams with OCR text{extra}; rule thresholds are provisional"));
    let analysis = analyse(&mut run.tracks, orig.as_deref(), rec.name(), rule);
    run.stdout.extend(analysis.lines.iter().cloned());
    run.orig_lang = orig;
    run.analysis = Some(analysis);
    run.exit_code = 0;
    run
}

/// <label>/subs.json (port spec section 5) and decide's "subtitles": numbers, classes and decisions only; no event
/// features, words or bigrams.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubsJson {
    /// The reading folder the OCR text came from (<DIR>/<slug>/<label>/<reading>/).
    pub reading: String,
    pub exit_code: i32,
    pub orig_lang: Option<String>,
    pub streams: Vec<SubsStreamJson>,
    pub pairs: Vec<PairJson>,
    /// "sub-<n>.ocr.jsonl" -> SHA-1 (lowercase hex) of the file's bytes, in stream order.
    pub text_sha1: Ordered<String>,
}

/// One stream of SubsJson.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubsStreamJson {
    pub n: i64,
    pub tag: String,
    pub codec: Option<String>,
    pub forced: bool,
    pub events: usize,
    /// "read" | "set aside" | "not read".
    pub status: String,
    /// Track.pairing (the pairing line).
    pub pairing: Option<String>,
    /// The measures (markers, class, language and corrections included); None when not read.
    pub m: Option<Measures>,
    pub decide: Option<Decision>,
    pub spec87: Option<String>,
    pub before_yes: Option<String>,
}

/// One pair of SubsJson: the two stream numbers, the pair measures flattened in, the subset phrase.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PairJson {
    pub a: i64,
    pub b: i64,
    #[serde(flatten)]
    pub pair: Pair,
    pub subset: String,
}

/// Lowercase hex SHA-1 of some bytes.
fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SubsJson of a run: per stream its facts, status, pairing, measures and the three outcomes (decide, spec87 and
/// before_yes as the table printed them; None when the run stopped before analyse), the printed pairs, and the
/// SHA-1 of every OCR text file the run read through `text_tree` (the streams with a pairing).
pub fn subs_json(run: &SubsRun, reading: &str, text_tree: &dyn Tree, text: &PathKey) -> SubsJson {
    let mut streams = Vec::new();
    let mut text_sha1: Ordered<String> = Ordered::new();
    for t in &run.tracks {
        let status = if t.has_text() {
            "read"
        } else if t.set_aside {
            "set aside"
        } else {
            "not read"
        };
        let (decision, today, before) = match &run.analysis {
            Some(analysis) => {
                let decision = analysis.decide.get(&t.n).cloned();
                let today = analysis.spec87.get(&t.n).cloned();
                let (_, _, _, _, before) = table_cells(analysis.decide.get(&t.n), analysis.spec87.get(&t.n));
                (decision, today, Some(before))
            }
            None => (None, None, None),
        };
        streams.push(SubsStreamJson {
            n: t.n,
            tag: t.tag.clone(),
            codec: t.codec.clone(),
            forced: t.forced,
            events: t.events.len(),
            status: status.to_string(),
            pairing: t.pairing.clone(),
            m: t.measured.as_ref().map(|measured| measured.m.clone()),
            decide: decision,
            spec87: today,
            before_yes: before,
        });
        if t.has_text() {
            let name = format!("sub-{}.ocr.jsonl", t.n);
            if let Ok(bytes) = text_tree.read(&text.join(&name)) {
                text_sha1.insert(name, sha1_hex(&bytes));
            }
        }
    }
    let pairs = match &run.analysis {
        Some(analysis) => analysis
            .pairs
            .iter()
            .map(|row| PairJson { a: row.a, b: row.b, pair: row.pair.clone(), subset: row.subset.clone() })
            .collect(),
        None => Vec::new(),
    };
    SubsJson {
        reading: reading.to_string(),
        exit_code: run.exit_code,
        orig_lang: run.orig_lang.clone(),
        streams,
        pairs,
        text_sha1,
    }
}
