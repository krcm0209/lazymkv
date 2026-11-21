//! Tests of `core::menu` (menu maps design 5.4): the shared fixture and the 49 shared validate cases (both copied byte
//! for byte from the Python writer's fixtures, so both readers answer the same questions), the version rule, the
//! reader rules, `effects_equal`, `map_level` and `shown_label`. Every map here is made up.

use serde_json::{json, Value};

use super::*;

const CASES: &str = include_str!("fixtures/menu-map-v1-cases.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

fn cases() -> Vec<Value> {
    serde_json::from_str(CASES).unwrap()
}

fn case(rule: &str) -> Value {
    cases().into_iter().find(|case| case["rule"] == rule).unwrap()
}

/// The value one step below `value`: a string step is an object key, a number step an array index.
fn step<'a>(value: &'a mut Value, key: &Value) -> &'a mut Value {
    match key {
        Value::String(key) => &mut value[key.as_str()],
        Value::Number(index) => &mut value[index.as_u64().unwrap() as usize],
        other => panic!("a path step is a key or an index, not {other}"),
    }
}

/// The map with the case's ops applied, as the Python test's `apply` does: each op sets or removes the value at its
/// path; the empty path replaces the root.
fn apply(map: &Value, ops: &Value) -> Value {
    let mut map = map.clone();
    for op in ops.as_array().unwrap() {
        let path = op["path"].as_array().unwrap();
        let Some((last, parents)) = path.split_last() else {
            map = op["value"].clone();
            continue;
        };
        let mut parent = &mut map;
        for key in parents {
            parent = step(parent, key);
        }
        if op.get("remove") == Some(&Value::Bool(true)) {
            match last {
                Value::String(key) => {
                    parent.as_object_mut().unwrap().remove(key.as_str());
                }
                other => {
                    parent.as_array_mut().unwrap().remove(other.as_u64().unwrap() as usize);
                }
            }
        } else {
            *step(parent, last) = op["value"].clone();
        }
    }
    map
}

fn strings(value: &Value) -> Vec<String> {
    value.as_array().unwrap().iter().map(|item| item.as_str().unwrap().to_string()).collect()
}

#[test]
fn every_shared_case_gives_the_writers_error_list() {
    let fixture = fixture();
    for case in cases() {
        let rule = case["rule"].as_str().unwrap();
        assert_eq!(validate(&apply(&fixture, &case["ops"])), strings(&case["errors"]), "{rule}");
    }
}

#[test]
fn the_shared_fixture_validates_and_reads() {
    assert_eq!(validate(&fixture()), Vec::<String>::new());
    let map = read(FIXTURE).unwrap();
    assert_eq!(map.format, FORMAT);
    assert_eq!(map.disc.slug, "made-up-1");
    assert_eq!(map.choices.len(), 9);
    let c006 = map.choices.iter().find(|choice| choice.id == "c006").unwrap();
    assert_eq!(c006.label, "Langue C — ÉTÉ");
    assert_eq!(
        (c006.kind, c006.surface, c006.applies, c006.check.level),
        (Kind::Subtitle, Surface::Popup, Applies::Immediately, Level::Read)
    );
    let c009 = map.choices.iter().find(|choice| choice.id == "c009").unwrap();
    assert_eq!(c009.effect, None);
    assert_eq!((c009.label_image.as_deref(), c009.differs_from_play), (None, None));
}

#[test]
fn a_newer_format_version_is_refused_with_one_plain_sentence() {
    let mut map = fixture();
    map["format"] = json!("lazymkv menu map v2");
    map["choices"][0]["kind"] = json!("video");
    assert_eq!(
        read(&map.to_string()),
        Err(vec!["this lazymkv reads only 'lazymkv menu map v1'; the map says it is 'lazymkv menu map v2'".to_string()])
    );
}

#[test]
fn a_map_whose_format_is_not_a_string_reads_as_every_problem() {
    let case = case("three_problems_listed");
    let errors = strings(&case["errors"]);
    assert_eq!(errors.len(), 3);
    assert_eq!(read(&apply(&fixture(), &case["ops"]).to_string()), Err(errors));
}

#[test]
fn text_that_is_not_json_is_refused() {
    let errors = read("{").unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].starts_with("not JSON: "), "{errors:?}");
    assert!(read(&format!("\u{feff}{FIXTURE}")).is_ok());
}

#[test]
fn unknown_keys_read() {
    let mut map = fixture();
    map["later"] = json!({"added": "by a later writer"});
    map["choices"][0]["hint"] = json!(["not known here"]);
    let read = read(&map.to_string()).unwrap();
    assert_eq!(read.choices[0].id, "c001");
}

#[test]
fn a_reader_rule_breach_is_refused_with_the_reader_sentence() {
    let mut map = fixture();
    map["disc"]["disc_id_sha1"] = json!(5);
    assert_eq!(validate(&map), Vec::<String>::new());
    let errors = read(&map.to_string()).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].starts_with("the map passes the format's rules but not this reader's: "), "{errors:?}");
}

#[test]
fn effects_equal_ignores_the_stream_number_when_subtitles_are_off() {
    let a = Effect { playlist: "00800".to_string(), audio: Some(1), subtitle: Some(1), subtitle_shown: false };
    let b = Effect { subtitle: Some(2), ..a.clone() };
    let c = Effect { subtitle: Some(2), subtitle_shown: true, ..a.clone() };
    assert!(effects_equal(&a, &b));
    assert!(!effects_equal(&a, &c));
    assert!(!effects_equal(&a, &Effect { audio: Some(2), ..a.clone() }));
    assert!(!effects_equal(&a, &Effect { playlist: "00801".to_string(), ..a.clone() }));
}

#[test]
fn map_level_is_the_lowest_of_play_and_the_choices() {
    let mut map = read(FIXTURE).unwrap();
    assert_eq!(map_level(&map), Level::Read);
    map.choices.retain(|choice| choice.id != "c006" && choice.id != "c009");
    assert_eq!(map_level(&map), Level::Checked);
    map.choices.retain(|choice| choice.id != "c003");
    assert_eq!(map_level(&map), Level::Replayed);
    map.choices.clear();
    assert_eq!(map_level(&map), Level::Read);
    assert!(Level::Read < Level::Checked && Level::Checked < Level::Replayed);
}

#[test]
fn shown_label_joins_section_and_label() {
    let map = read(FIXTURE).unwrap();
    let mut c002 = map.choices[1].clone();
    assert_eq!(c002.id, "c002");
    assert_eq!(c002.shown_label(), "SETUP > AUDIO: Language B 5.1");
    c002.section = String::new();
    assert_eq!(c002.shown_label(), "Language B 5.1");
}

#[test]
fn the_cases_file_holds_49_unique_rules_each_with_an_error() {
    let cases = cases();
    let mut rules: Vec<&str> = cases.iter().map(|case| case["rule"].as_str().unwrap()).collect();
    rules.sort();
    rules.dedup();
    assert_eq!((cases.len(), rules.len()), (49, 49));
    assert!(cases.iter().all(|case| !strings(&case["errors"]).is_empty()));
}
