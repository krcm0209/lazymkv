//! What a menu calls an audio choice (menu maps design 5.4a, rulings MR-6, MR-7 and MR-10 of the menu-role plan): the
//! role words of a choice's text, the "net" of words that may mean a switch, the per-choice off-decisions file, and the
//! role of a stream from the choices that label it. Words are matched as the measurement's Python matched them, without
//! regard to case, with a word edge being a neighbour that is neither alphanumeric nor `_`. Evidence only; no I/O.

use std::collections::{BTreeMap, BTreeSet};

use unicode_normalization::UnicodeNormalization;

/// What a menu calls an audio stream (design 5.4a): serde and `as_str` give "commentary", "audio_description",
/// "programme" (MR-10). Ordered as listed, so a set of roles has one order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioRole {
    Commentary,
    AudioDescription,
    Programme,
}

impl AudioRole {
    /// The role's word (the serde spelling).
    pub fn as_str(self) -> &'static str {
        match self {
            AudioRole::Commentary => "commentary",
            AudioRole::AudioDescription => "audio_description",
            AudioRole::Programme => "programme",
        }
    }
}

/// True for a character Python's `str.isspace` (and so `\s`) counts as white space: Rust's set and the four separators
/// `\x1c` to `\x1f`.
fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `text` with every run of white space as one space, trimmed (Python's `re.sub(r'\s+', ' ', text).strip()`).
fn collapse(text: &str) -> String {
    text.split(is_space).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The measurement's text of a choice: the innermost heading of `section` (after the last " > ", trimmed), ": ", the
/// label; the label alone when that heading is empty; every '|' written '/'; white space runs collapsed to one space;
/// trimmed.
pub fn role_text(section: &str, label: &str) -> String {
    let heading = section.rsplit(" > ").next().unwrap_or("").trim_matches(is_space);
    let text = if heading.is_empty() { label.to_string() } else { format!("{heading}: {label}") };
    collapse(&text.replace('|', "/"))
}

/// True when the character is a word character in Python's sense: alphanumeric or `_`.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// True when `hay` (already lower-cased) holds `word`; with `edges` only where neither neighbour is a word character.
fn holds(hay: &str, word: &str, edges: bool) -> bool {
    hay.match_indices(word).any(|(at, found)| {
        !edges
            || (!hay[..at].chars().next_back().is_some_and(is_word)
                && !hay[at + found.len()..].chars().next().is_some_and(is_word))
    })
}

/// The roles whose words `text` holds, in the enum's order, each once.
pub fn audio_roles_in(text: &str) -> Vec<AudioRole> {
    let hay = text.to_lowercase();
    let anywhere = |words: &[&str]| words.iter().any(|word| holds(&hay, word, false));
    let edged = |words: &[&str]| words.iter().any(|word| holds(&hay, word, true));
    let mut roles = Vec::new();
    if anywhere(&["commentar"]) {
        roles.push(AudioRole::Commentary);
    }
    if anywhere(&["audio descri", "audio-descri", "descriptive", "visually impaired", "visually-impaired"])
        || edged(&["described", "dvs", "ad"])
    {
        roles.push(AudioRole::AudioDescription);
    }
    if edged(&["main", "programm", "programme"]) {
        roles.push(AudioRole::Programme);
    }
    roles
}

/// The words that may mean a switch is off or a track is absent, each matched between word edges.
const NET: [&str; 10] = ["off", "no", "none", "not", "without", "disable", "disabled", "stop", "hide", "hidden"];

/// True when the lower-cased `hay` holds a net word.
fn net_catches(hay: &str) -> bool {
    NET.iter().any(|word| holds(hay, word, true))
}

/// True when `text` holds a role word and the net catches it or its NFKC lower-cased form (MR-7).
pub fn needs_decision(text: &str) -> bool {
    if audio_roles_in(text).is_empty() {
        return false;
    }
    net_catches(&text.to_lowercase()) || net_catches(&collapse(&text.nfkc().collect::<String>().to_lowercase()))
}

/// The off-decisions file's name in a map's folder (MR-6).
pub const OFF_FILE_NAME: &str = "menu-off.tsv";

/// The owner's decision on one choice whose text the net catches (MR-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffDecision {
    /// The choice states no role for the stream it leaves playing: its words are not read.
    Off,
    /// The choice's words are read as any other's.
    Role,
}

impl OffDecision {
    /// The decision's word in the file.
    fn word(self) -> &'static str {
        match self {
            OffDecision::Off => "off",
            OffDecision::Role => "role",
        }
    }
}

/// The decisions of one map's `menu-off.tsv`, by choice id. `Default` is no decision.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OffDecisions {
    by_id: BTreeMap<String, OffDecision>,
}

impl OffDecisions {
    /// The decision on choice `id`, if any.
    pub fn get(&self, id: &str) -> Option<OffDecision> {
        self.by_id.get(id).copied()
    }

    /// Every decided choice id, ascending.
    pub fn ids(&self) -> Vec<&str> {
        self.by_id.keys().map(String::as_str).collect()
    }
}

/// True for a choice id: `c` and exactly three ASCII digits.
fn is_choice_id(id: &str) -> bool {
    id.len() == 4 && id.starts_with('c') && id[1..].bytes().all(|byte| byte.is_ascii_digit())
}

/// The decisions in `text` (a `menu-off.tsv`; MR-6): UTF-8, a leading byte-order mark dropped, lines split on `\n` with
/// a trailing `\r` left off. A blank line and a line starting with `#` are skipped; every other line is
/// `<decision><TAB><choice id>` with the decision `off` or `role`. Err is every problem, one sentence each, in line
/// order; a line repeating an earlier one is dropped.
pub fn parse_off(text: &str) -> Result<OffDecisions, Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let repr = crate::recording::py_repr_str;
    let (mut by_id, mut first_line, mut errors) = (BTreeMap::new(), BTreeMap::new(), Vec::new());
    for (index, line) in text.split('\n').enumerate() {
        let (number, line) = (index + 1, line.strip_suffix('\r').unwrap_or(line));
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [word, id] = fields[..] else {
            errors.push(format!("line {number}: expected \"off<TAB><choice id>\" or \"role<TAB><choice id>\""));
            continue;
        };
        let decision = match word {
            "off" => Some(OffDecision::Off),
            "role" => Some(OffDecision::Role),
            _ => {
                errors.push(format!("line {number}: unknown decision {} (use \"off\" or \"role\")", repr(word)));
                None
            }
        };
        if !is_choice_id(id) {
            errors.push(format!("line {number}: {} is not a choice id (c and three digits)", repr(id)));
            continue;
        }
        let Some(decision) = decision else { continue };
        match by_id.get(id) {
            Some(&earlier) if earlier != decision => errors.push(format!(
                "line {number}: {id} is decided {} here and {} on line {}",
                decision.word(),
                OffDecision::word(earlier),
                first_line[id]
            )),
            Some(_) => {}
            None => {
                by_id.insert(id.to_string(), decision);
                first_line.insert(id, number);
            }
        }
    }
    if errors.is_empty() {
        Ok(OffDecisions { by_id })
    } else {
        Err(errors)
    }
}

/// What the choices that label one stream say it is.
#[derive(Clone, Debug, PartialEq)]
pub enum RoleReading {
    /// Exactly one role is read.
    Role(AudioRole),
    /// No text states a role.
    Silent,
    /// Several roles are read, in the enum's order.
    Several(Vec<AudioRole>),
    /// These choice ids (in the order given) need an off-decision and have none; whatever else the texts say, the
    /// stream has no role (MR-6).
    Undecided(Vec<String>),
}

/// The role of a stream from the choices that label it; `choices` are (choice id, `role_text`) in map order. A text
/// with no role word is skipped; one decided `off` is skipped, one decided `role` is read; one the net catches with no
/// decision is undecided and its words are not read. Any undecided id gives `Undecided` (MR-6).
pub fn stream_role(choices: &[(&str, &str)], off: &OffDecisions) -> RoleReading {
    let (mut undecided, mut roles) = (Vec::new(), BTreeSet::new());
    for &(id, text) in choices {
        let found = audio_roles_in(text);
        if found.is_empty() {
            continue;
        }
        match off.get(id) {
            Some(OffDecision::Off) => {}
            Some(OffDecision::Role) => roles.extend(found),
            None if needs_decision(text) => undecided.push(id.to_string()),
            None => roles.extend(found),
        }
    }
    if !undecided.is_empty() {
        return RoleReading::Undecided(undecided);
    }
    let mut roles: Vec<AudioRole> = roles.into_iter().collect();
    match roles.len() {
        0 => RoleReading::Silent,
        1 => RoleReading::Role(roles.remove(0)),
        _ => RoleReading::Several(roles),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared cases' value at `key`.
    fn cases(key: &str) -> Vec<serde_json::Value> {
        let all: serde_json::Value = serde_json::from_str(crate::menu::ROLE_CASES).unwrap();
        all[key].as_array().unwrap().clone()
    }

    /// The off-decisions of `text`, which must parse.
    fn off(text: &str) -> OffDecisions {
        parse_off(text).unwrap()
    }

    /// Every `label_text` case: the measurement's text of a choice.
    #[test]
    fn the_shared_cases_give_the_measurements_label_text() {
        for case in cases("label_text") {
            let (section, label) = (case["section"].as_str().unwrap(), case["label"].as_str().unwrap());
            assert_eq!(role_text(section, label), case["text"].as_str().unwrap(), "{case}");
        }
    }

    /// Every `roles` case: the role words and the net.
    #[test]
    fn the_shared_cases_give_the_measurements_roles_and_net() {
        for case in cases("roles") {
            let text = case["text"].as_str().unwrap();
            let roles: Vec<&str> = audio_roles_in(text).iter().map(|role| role.as_str()).collect();
            let want: Vec<&str> = case["roles"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
            assert_eq!(roles, want, "{case}");
            assert_eq!(needs_decision(text), case["needs_decision"].as_bool().unwrap(), "{case}");
        }
    }

    /// The three spellings, through `as_str` and serde both ways.
    #[test]
    fn a_role_spells_as_serde_writes_it() {
        let spellings = [
            (AudioRole::Commentary, "commentary"),
            (AudioRole::AudioDescription, "audio_description"),
            (AudioRole::Programme, "programme"),
        ];
        for (role, word) in spellings {
            assert_eq!(role.as_str(), word);
            assert_eq!(serde_json::to_string(&role).unwrap(), format!("\"{word}\""));
            assert_eq!(serde_json::from_str::<AudioRole>(&format!("\"{word}\"")).unwrap(), role);
        }
    }

    /// A byte-order mark, comments, blank lines and CRLF line ends are read.
    #[test]
    fn an_off_file_reads_decisions_comments_and_a_byte_order_mark() {
        let decisions = off("\u{feff}# why\r\noff\tc012\r\n\nrole\tc013\n");
        assert_eq!(decisions.get("c012"), Some(OffDecision::Off));
        assert_eq!(decisions.get("c013"), Some(OffDecision::Role));
        assert_eq!(decisions.get("c014"), None);
        assert_eq!(decisions.ids(), vec!["c012", "c013"]);
    }

    /// Every problem is one sentence, in line order; a repeated line is dropped.
    #[test]
    fn every_problem_of_an_off_file_is_listed() {
        let errors = parse_off("off c012\nmaybe\tc012\noff\tc12\noff\tc001\nrole\tc001\noff\tc001\n").unwrap_err();
        assert_eq!(
            errors,
            vec![
                "line 1: expected \"off<TAB><choice id>\" or \"role<TAB><choice id>\"",
                "line 2: unknown decision 'maybe' (use \"off\" or \"role\")",
                "line 3: 'c12' is not a choice id (c and three digits)",
                "line 5: c001 is decided role here and off on line 4",
            ]
        );
    }

    /// One role read is the stream's role.
    #[test]
    fn one_role_is_the_streams_role() {
        let choices = [("c001", "AUDIO: Language A"), ("c002", "AUDIO: Commentary by Person A")];
        assert_eq!(stream_role(&choices, &OffDecisions::default()), RoleReading::Role(AudioRole::Commentary));
    }

    /// No role word anywhere is silent.
    #[test]
    fn texts_without_role_words_are_silent() {
        let choices = [("c001", "AUDIO: Language A"), ("c002", "AUDIO: Language B 5.1")];
        assert_eq!(stream_role(&choices, &OffDecisions::default()), RoleReading::Silent);
        assert_eq!(stream_role(&[], &OffDecisions::default()), RoleReading::Silent);
    }

    /// Two roles are several, in the enum's order whatever the choices' order.
    #[test]
    fn two_different_roles_are_several_in_enum_order() {
        let choices = [("c001", "AUDIO: Main Feature"), ("c002", "AUDIO: Commentary by Person A")];
        assert_eq!(
            stream_role(&choices, &OffDecisions::default()),
            RoleReading::Several(vec![AudioRole::Commentary, AudioRole::Programme])
        );
    }

    /// `off` skips a text's words, `role` reads them.
    #[test]
    fn an_off_decision_skips_the_words_and_a_role_decision_reads_them() {
        let choices = [("c001", "COMMENTARY: Off")];
        assert_eq!(stream_role(&choices, &off("off\tc001\n")), RoleReading::Silent);
        assert_eq!(stream_role(&choices, &off("role\tc001\n")), RoleReading::Role(AudioRole::Commentary));
    }

    /// An undecided text beats every other text (MR-6).
    #[test]
    fn an_undecided_text_leaves_the_stream_without_a_role() {
        let choices = [("c001", "AUDIO: Commentary by Person A"), ("c002", "COMMENTARY: Off")];
        assert_eq!(
            stream_role(&choices, &OffDecisions::default()),
            RoleReading::Undecided(vec!["c002".to_string()])
        );
    }
}
