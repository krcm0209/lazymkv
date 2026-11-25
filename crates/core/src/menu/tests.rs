//! Tests of `core::menu` (menu maps design 5.4): the shared fixture and the 49 shared validate cases (both copied byte
//! for byte from the Python writer's fixtures, so both readers answer the same questions), the version rule, the
//! reader rules, `effects_equal`, `map_level` and `shown_label`; then the map against the made-up scan
//! `testsupport::menu_scan_text`: stream numbers, identity, the check and the title block. Every map and scan here is
//! made up.

use serde_json::{json, Value};

use super::*;
use crate::disc::{Disc, Title};
use crate::testsupport::{menu_scan_text, scan_text};

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
    assert_eq!(read(&format!("\u{feff}{FIXTURE}")), read(FIXTURE));
}

#[test]
fn unknown_keys_read() {
    let mut map = fixture();
    map["later"] = json!({"added": "by a later writer"});
    map["choices"][0]["hint"] = json!(["not known here"]);
    let got = read(&map.to_string()).unwrap();
    assert_eq!(got.choices[0].id, "c001");
}

#[test]
fn a_reader_rule_breach_is_refused_with_the_reader_sentence() {
    let mut map = fixture();
    map["disc"]["disc_id_sha1"] = json!(5);
    assert_eq!(validate(&map), Vec::<String>::new());
    let errors = read(&map.to_string()).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].starts_with("the map passes the format's rules but not this reader's: "), "{errors:?}");
    // The writer's list wins: with a writer-rule breach too, only the writer's sentence is given.
    map["choices"][0]["kind"] = json!("video");
    let kind = "choices[0] (c001): kind 'video' is not one of audio, subtitle, version, bonus, other";
    assert_eq!(read(&map.to_string()), Err(vec![kind.to_string()]));
}

#[test]
fn every_word_is_spelled_as_serde_spells_it() {
    let spelled = |value: serde_json::Value, word: &str| assert_eq!(value, json!(word));
    for kind in Kind::ALL {
        spelled(serde_json::to_value(kind).unwrap(), kind.word());
    }
    for surface in Surface::ALL {
        spelled(serde_json::to_value(surface).unwrap(), surface.word());
    }
    for level in Level::ALL {
        spelled(serde_json::to_value(level).unwrap(), level.word());
    }
    for applies in Applies::ALL {
        spelled(serde_json::to_value(applies).unwrap(), applies.word());
    }
}

#[test]
fn the_typed_effects_equal_agrees_with_differs_from_play_on_the_fixture() {
    let map = read(FIXTURE).unwrap();
    let play = map.play.effect.as_ref().unwrap();
    let mut compared = 0;
    for choice in &map.choices {
        if let Some(effect) = &choice.effect {
            assert_eq!(choice.differs_from_play, Some(!effects_equal(effect, play)), "{}", choice.id);
            compared += 1;
        }
    }
    assert_eq!(compared, 8);
}

#[test]
fn the_fixtures_are_kept_as_bytes_on_every_checkout() {
    let attributes = include_str!("../../../../.gitattributes");
    assert!(attributes.lines().any(|line| line == "crates/core/src/menu/fixtures/** -text"), "{attributes}");
    assert!(!FIXTURE.contains('\r') && !CASES.contains('\r'));
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

// ------------------------------------------------------------------------------------------------------------------
// The map against a scan (design 5.3 and 5.4, M3-1 to M3-6), on the made-up scan `testsupport::menu_scan_text`.
// ------------------------------------------------------------------------------------------------------------------

/// The fixture map's disc ID.
const ID: &str = "0123456789ABCDEF0123456789ABCDEF01234567";

/// The last line of a check with these counts.
fn check_line(verdict: &str, counts: [usize; 6]) -> String {
    let [entries, on_scan, not_in_scan, no_effect, warnings, failed] = counts;
    format!(
        "CHECK {verdict} entries={entries} on_scan={on_scan} not_in_scan={not_in_scan} no_effect={no_effect} \
         warnings={warnings} failed={failed}; streams of not_in_scan entries were not checked"
    )
}

fn scan_disc(text: &str) -> Disc {
    crate::disc::disc(&crate::robot::parse(text)).unwrap()
}

fn menu_disc() -> Disc {
    scan_disc(&menu_scan_text())
}

fn fixture_map() -> MenuMap {
    read(FIXTURE).unwrap()
}

/// The fixture map with `edit` applied to its JSON; the edited map passes the writer's rules.
fn edited(edit: impl FnOnce(&mut Value)) -> MenuMap {
    let mut map = fixture();
    edit(&mut map);
    assert_eq!(validate(&map), Vec::<String>::new());
    read(&map.to_string()).unwrap()
}

fn matching() -> Identity {
    identity(&fixture_map(), &DiscId::Known(ID.to_string()))
}

fn label(id: &str, label: &str, level: Level, differs_from_play: Option<bool>) -> MenuLabel {
    MenuLabel { id: id.to_string(), label: label.to_string(), level, differs_from_play }
}

fn other(playlist: &str, entries: u32, in_scan: bool) -> OtherPlaylist {
    OtherPlaylist { playlist: playlist.to_string(), entries, in_scan }
}

fn ids(labels: &[MenuLabel]) -> Vec<&str> {
    labels.iter().map(|label| label.id.as_str()).collect()
}

/// Each row's stream with the ids of its labels.
fn rows(rows: &[StreamLabels]) -> Vec<(u32, Vec<&str>)> {
    rows.iter().map(|row| (row.stream, ids(&row.labels))).collect()
}

fn title<'a>(disc: &'a Disc, number: u32) -> &'a Title {
    disc.title(number).unwrap()
}

#[test]
fn map_numbers_skip_core_audio_and_derived_subtitles() {
    let disc = menu_disc();
    assert_eq!(map_streams(title(&disc, 0)), MapStreams { audio: vec![1, 3, 4], subtitles: vec![5, 7, 8] });
    assert_eq!(map_streams(title(&disc, 1)), MapStreams { audio: vec![1], subtitles: vec![] });
    let numbers = |disc: &Disc, playlist: &str| -> Vec<u32> {
        titles_for(disc, playlist).iter().map(|title| title.number).collect()
    };
    assert_eq!((numbers(&disc, "00800"), numbers(&disc, "01001")), (vec![0], vec![1]));
    assert_eq!(numbers(&disc, "00801"), Vec::<u32>::new());
    let mut upper = disc.clone();
    upper.titles.get_mut(&1).unwrap().source_file = Some("01001.MPLS".to_string());
    assert_eq!(numbers(&upper, "01001"), vec![1]);
}

#[test]
fn every_title_of_a_shared_source_file_is_checked() {
    let audio = |language| [(1, "Audio"), (3, language), (22, "0")];
    let subtitle = |language, flags| [(1, "Subtitles"), (3, language), (22, flags)];
    let (a1, a2, a3) = (audio("eng"), [(1, "Audio"), (3, "eng"), (22, "256")], audio("spa"));
    let (s5, s6, s7, s8) = (subtitle("eng", "0"), subtitle("eng", "2048"), subtitle("fra", "0"), subtitle("eng", "0"));
    let extra = scan_text(
        &[],
        &[(
            2,
            &[(16, "00800.mpls")],
            &[(0, &[(1, "Video")]), (1, &a1), (2, &a2), (3, &a3), (5, &s5), (6, &s6), (7, &s7), (8, &s8)],
        )],
    );
    let disc = scan_disc(&format!("{}{extra}", menu_scan_text()));
    assert_eq!(map_streams(title(&disc, 2)), MapStreams { audio: vec![1, 3], subtitles: vec![5, 7, 8] });
    let report = check(&fixture_map(), &disc, matching());
    let at = report.lines.iter().position(|line| line.starts_with("c003: ")).unwrap();
    assert_eq!(
        report.lines[at..at + 2],
        [
            "c003: playlist 00800, title 0: audio 3 = stream 4, subtitle 1 off",
            "c003: playlist 00800, title 2: audio 3 OUT OF RANGE (2 audio streams), subtitle 1 off",
        ]
    );
    let c002 = "c002: playlist 00800, title 2: audio 2 = stream 3, subtitle 3 = stream 8";
    assert!(report.lines.iter().any(|line| line == c002), "{:?}", report.lines);
    assert_eq!((report.on_scan, report.failed), (8, 1));
    assert_eq!(report.lines.last().unwrap(), &check_line("FAILED", [10, 8, 1, 1, 0, 1]));
    // The block of title 2 is made on its own streams: c003's audio 3 names none of its two.
    let block = title_block(&fixture_map(), &disc, title(&disc, 2), &matching());
    assert_eq!(rows(&block.audio), [(1, vec!["c001"]), (3, vec!["c002"])]);
    assert_eq!(rows(&block.subtitles), [(7, vec!["c004"]), (8, vec!["c006"])]);
    assert_eq!(block.out_of_range, ["c003"]);
    // A warning counts once on each title line: c005's leftover beyond the table, on titles 0 and 2.
    let map = edited(|map| map["choices"][4]["effect"]["subtitle"] = json!(7));
    let report = check(&map, &disc, matching());
    let c005: Vec<&String> = report.lines.iter().filter(|line| line.starts_with("c005: ")).collect();
    assert_eq!(
        c005,
        [
            "c005: playlist 00800, title 0: audio 1 = stream 1, subtitle 7 off, leftover beyond 3 (warning)",
            "c005: playlist 00800, title 2: audio 1 = stream 1, subtitle 7 off, leftover beyond 3 (warning)",
        ]
    );
    assert_eq!((report.warnings, report.failed), (2, 1));
    assert_eq!(report.lines.last().unwrap(), &check_line("FAILED", [10, 8, 1, 1, 2, 1]));
}

#[test]
fn the_fixture_check_prints_these_lines() {
    let report = check(&fixture_map(), &menu_disc(), matching());
    assert_eq!(report.identity, Identity::Matches(ID.to_string()));
    assert_eq!(
        report.lines,
        [
            "identity: disc_id matches (0123456789ABCDEF0123456789ABCDEF01234567)",
            "listing_sig: not checked (lazymkv does not recompute it)",
            "play: playlist 00800, title 0: audio 1 = stream 1, subtitle 1 off",
            "c001: playlist 00800, title 0: audio 1 = stream 1, subtitle 1 off",
            "c002: playlist 00800, title 0: audio 2 = stream 3, subtitle 3 = stream 8",
            "c003: playlist 00800, title 0: audio 3 = stream 4, subtitle 1 off",
            "c004: playlist 00800, title 0: audio 1 = stream 1, subtitle 2 = stream 7",
            "c005: playlist 00800, title 0: audio 1 = stream 1, subtitle 2 off",
            "c006: playlist 00800, title 0: audio 1 = stream 1, subtitle 3 = stream 8",
            "c007: playlist 01001, title 1: audio 1 = stream 1, subtitle none",
            "c008: playlist 00801 not in the scan",
            "c009: no effect recorded",
            concat!(
                "CHECK ok entries=10 on_scan=8 not_in_scan=1 no_effect=1 warnings=0 failed=0; ",
                "streams of not_in_scan entries were not checked"
            ),
        ]
    );
    let counts = [report.entries, report.on_scan, report.not_in_scan, report.no_effect, report.warnings, report.failed];
    assert_eq!(counts, [10, 8, 1, 1, 0, 0]);
}

#[test]
fn an_out_of_range_audio_number_fails() {
    let map = edited(|map| map["choices"][2]["effect"]["audio"] = json!(9));
    let report = check(&map, &menu_disc(), matching());
    assert_eq!(
        report.lines[5],
        "c003: playlist 00800, title 0: audio 9 OUT OF RANGE (3 audio streams), subtitle 1 off"
    );
    assert_eq!((report.warnings, report.failed), (0, 1));
    assert_eq!(report.lines.last().unwrap(), &check_line("FAILED", [10, 8, 1, 1, 0, 1]));
    let disc = menu_disc();
    let block = title_block(&map, &disc, title(&disc, 0), &matching());
    assert_eq!(block.out_of_range, ["c003"]);
    assert_eq!(rows(&block.audio), [(1, vec!["c001"]), (3, vec!["c002"])]);
    let map = edited(|map| {
        map["choices"][2]["effect"]["audio"] = json!(0);
        map["choices"][5]["effect"]["subtitle"] = json!(4);
    });
    let block = title_block(&map, &disc, title(&disc, 0), &matching());
    assert_eq!(block.out_of_range, ["c003", "c006"]);
    assert_eq!(rows(&block.audio), [(1, vec!["c001"]), (3, vec!["c002"])]);
    assert_eq!(rows(&block.subtitles), [(7, vec!["c004"])]);
    let report = check(&map, &menu_disc(), matching());
    assert_eq!(
        report.lines[5],
        "c003: playlist 00800, title 0: audio 0 OUT OF RANGE (3 audio streams), subtitle 1 off"
    );
    assert_eq!(
        report.lines[8],
        "c006: playlist 00800, title 0: audio 1 = stream 1, subtitle 4 OUT OF RANGE (3 subtitle streams)"
    );
    assert_eq!(report.lines.last().unwrap(), &check_line("FAILED", [10, 8, 1, 1, 0, 2]));
}

#[test]
fn a_leftover_subtitle_beyond_the_table_is_a_warning_not_a_failure() {
    let map = edited(|map| map["choices"][4]["effect"]["subtitle"] = json!(7));
    let disc = menu_disc();
    let report = check(&map, &disc, matching());
    assert_eq!(
        report.lines[7],
        "c005: playlist 00800, title 0: audio 1 = stream 1, subtitle 7 off, leftover beyond 3 (warning)"
    );
    assert_eq!((report.warnings, report.failed), (1, 0));
    assert_eq!(report.lines.last().unwrap(), &check_line("ok", [10, 8, 1, 1, 1, 0]));
    let block = title_block(&map, &disc, title(&disc, 0), &matching());
    assert_eq!(ids(&block.subtitles_off), ["c005"]);
    let shown: Vec<(u32, Vec<&str>)> = block.subtitles.iter().map(|row| (row.stream, ids(&row.labels))).collect();
    assert_eq!(shown, [(7, vec!["c004"]), (8, vec!["c006"])]);
    assert_eq!(block.out_of_range, Vec::<String>::new());
    let map = edited(|map| map["choices"][4]["effect"]["subtitle"] = json!(0));
    let report = check(&map, &disc, matching());
    assert_eq!(
        report.lines[7],
        "c005: playlist 00800, title 0: audio 1 = stream 1, subtitle 0 off, leftover below 1 (warning)"
    );
    assert_eq!(report.lines.last().unwrap(), &check_line("ok", [10, 8, 1, 1, 1, 0]));
}

#[test]
fn another_discs_map_fails_the_identity() {
    let map = fixture_map();
    let theirs = identity(&map, &DiscId::Known("FFFF".to_string()));
    assert_eq!(theirs, Identity::Differs { map: ID.to_string(), disc: "FFFF".to_string() });
    let report = check(&map, &menu_disc(), theirs);
    assert_eq!(
        report.lines[0],
        "identity: disc_id differs: the map's is 0123456789ABCDEF0123456789ABCDEF01234567, the disc's is FFFF"
    );
    assert_eq!(report.failed, 1);
    assert_eq!(report.lines.last().unwrap(), &check_line("FAILED", [10, 8, 1, 1, 0, 1]));
    assert_eq!(identity(&map, &DiscId::Known(ID.to_lowercase())), Identity::Matches(ID.to_string()));
}

#[test]
fn identity_is_not_checked_without_both_ids() {
    let map = fixture_map();
    let absent = identity(&map, &DiscId::Absent(NO_ID_IN_SCAN_FILE.to_string()));
    assert_eq!(absent, Identity::NotChecked(NO_ID_IN_SCAN_FILE.to_string()));
    let report = check(&map, &menu_disc(), absent);
    assert_eq!(report.lines[0], "identity: not checked: a scan file carries no disc ID; give the recording folder");
    assert_eq!(report.lines.last().unwrap(), &check_line("ok", [10, 8, 1, 1, 0, 0]));
    for reason in [NO_META, META_WITHOUT_ID] {
        assert_eq!(identity(&map, &DiscId::Absent(reason.to_string())), Identity::NotChecked(reason.to_string()));
    }
    assert_eq!(NO_META, "the recording has no meta.json");
    assert_eq!(META_WITHOUT_ID, "the recording's meta.json has no disc_id");
    let map = edited(|map| map["disc"]["disc_id_sha1"] = Value::Null);
    let unknown = identity(&map, &DiscId::Known(ID.to_string()));
    assert_eq!(unknown, Identity::NotChecked(MAP_WITHOUT_ID.to_string()));
    assert_eq!(unknown.text(), "not checked: the map has no disc_id_sha1");
    // The map's ID is read as the recording's is: trimmed, and an empty or blank one is no ID.
    for blank in ["", "   "] {
        let map = edited(|map| map["disc"]["disc_id_sha1"] = json!(blank));
        assert_eq!(identity(&map, &DiscId::Known(ID.to_string())), Identity::NotChecked(MAP_WITHOUT_ID.to_string()));
    }
    let map = edited(|map| map["disc"]["disc_id_sha1"] = json!(format!(" {ID} ")));
    assert_eq!(identity(&map, &DiscId::Known(ID.to_string())), Identity::Matches(ID.to_string()));
    let theirs = identity(&map, &DiscId::Known("FFFF".to_string()));
    assert_eq!(theirs, Identity::Differs { map: ID.to_string(), disc: "FFFF".to_string() });
}

#[test]
fn map_text_cannot_break_or_forge_a_check_line() {
    let forged = "00801\nCHECK ok entries=0 on_scan=0 not_in_scan=0 no_effect=0 warnings=0 failed=0";
    let map = edited(|map| map["choices"][7]["effect"]["playlist"] = json!(forged));
    let report = check(&map, &menu_disc(), matching());
    assert_eq!(
        report.lines[10],
        "c008: playlist '00801\\nCHECK ok entries=0 on_scan=0 not_in_scan=0 no_effect=0 warnings=0 failed=0' not in \
         the scan"
    );
    let printed = report.lines.join("\n");
    assert_eq!(printed.lines().count(), 13, "{printed}");
    assert_eq!(printed.lines().filter(|line| line.starts_with("CHECK ")).count(), 1, "{printed}");
    // Five ASCII digits print as they are; anything else is quoted.
    assert_eq!(playlist_text("00800"), "00800");
    assert_eq!([playlist_text("800"), playlist_text("0080a"), playlist_text("")], ["'800'", "'0080a'", "''"]);
}

#[test]
fn a_disc_id_cannot_break_or_forge_a_check_line() {
    let forged = "FFFF\nCHECK ok entries=0 on_scan=0 not_in_scan=0 no_effect=0 warnings=0 failed=0";
    let quoted = "'FFFF\\nCHECK ok entries=0 on_scan=0 not_in_scan=0 no_effect=0 warnings=0 failed=0'";
    // The recording's ID (a meta.json `disc_id`) differs from the map's.
    let map = fixture_map();
    let report = check(&map, &menu_disc(), identity(&map, &DiscId::Known(forged.to_string())));
    assert_eq!(report.lines[0], format!("identity: disc_id differs: the map's is {ID}, the disc's is {quoted}"));
    let printed = report.lines.join("\n");
    assert_eq!(printed.lines().count(), 13, "{printed}");
    assert_eq!(printed.lines().filter(|line| line.starts_with("CHECK ")).count(), 1, "{printed}");
    assert_eq!(report.lines.last().unwrap(), &check_line("FAILED", [10, 8, 1, 1, 0, 1]));
    // The map's ID, matched by the same text in the recording's, and differing from another.
    let map = edited(|map| map["disc"]["disc_id_sha1"] = json!(forged));
    let ours = identity(&map, &DiscId::Known(forged.to_lowercase()));
    assert_eq!(ours.text(), format!("disc_id matches ({quoted})"));
    let report = check(&map, &menu_disc(), ours);
    assert_eq!(report.lines.join("\n").lines().count(), 13);
    let theirs = identity(&map, &DiscId::Known(ID.to_string()));
    assert_eq!(theirs.text(), format!("disc_id differs: the map's is {quoted}, the disc's is {ID}"));
    // Decide's block holds the same text.
    let disc = menu_disc();
    assert_eq!(title_block(&map, &disc, title(&disc, 0), &theirs).identity, theirs.text());
}

#[test]
fn the_print_helpers_quote_only_text_that_could_break_a_line() {
    // ASCII hex digits print as they are; any other ID is quoted.
    assert_eq!([id_text(ID), id_text("ffff")], [ID, "ffff"]);
    assert_eq!([id_text(""), id_text("AB CD"), id_text("ABCDEFG")], ["''", "'AB CD'", "'ABCDEFG'"]);
    // Free text prints as it is unless it holds a control character, U+2028 or U+2029: what the writer keeps out of a
    // label.
    for plain in ["SETUP > AUDIO: Language A 5.1", "Langue C — ÉTÉ", "", "a 'quoted' \"word\""] {
        assert_eq!(line_text(plain), plain);
    }
    let breaking = [
        ("a\nb", "'a\\nb'"),
        ("a\rb", "'a\\rb'"),
        ("a\tb", "'a\\tb'"),
        ("a\u{b}b", "'a\\x0bb'"),
        ("a\u{85}b", "'a\\x85b'"),
        ("a\u{2028}b", "'a\\u2028b'"),
        ("a\u{2029}b", "'a\\u2029b'"),
    ];
    for (text, quoted) in breaking {
        assert_eq!(line_text(text), quoted, "{text:?}");
        // The writer refuses each of them in a label.
        let mut map = fixture();
        map["play"]["label"] = json!(text);
        let problems = validate(&map);
        let refused = "label has a tab, line break, control character or lone surrogate";
        assert!(problems.iter().any(|problem| problem.ends_with(refused)), "{text:?}: {problems:?}");
    }
}

#[test]
fn other_playlists_that_differ_in_ascii_case_alone_share_one_row() {
    let map = edited(|map| {
        map["choices"][6]["effect"]["playlist"] = json!("Extra");
        map["choices"][7]["effect"]["playlist"] = json!("EXTRA");
    });
    let disc = menu_disc();
    let block = title_block(&map, &disc, title(&disc, 0), &matching());
    assert_eq!(block.other_playlists, [other("Extra", 2, false)]);
}

#[test]
fn the_title_block_labels_only_streams_of_the_choices_own_kind() {
    let (map, disc) = (fixture_map(), menu_disc());
    let streams = |stream: u32, labels: Vec<MenuLabel>| StreamLabels { stream, labels };
    let c007 = label("c007", "EXTRAS: Bonus 1", Level::Replayed, Some(true));
    let block = title_block(&map, &disc, title(&disc, 0), &matching());
    let want = MenuBlock {
        map: "made-up-1".to_string(),
        map_level: Level::Read,
        identity: "disc_id matches (0123456789ABCDEF0123456789ABCDEF01234567)".to_string(),
        playlist: Some("00800".to_string()),
        starts: vec![label("play", "PLAY", Level::Replayed, None)],
        audio: vec![
            streams(1, vec![label("c001", "SETUP > AUDIO: Language A 5.1", Level::Replayed, Some(false))]),
            streams(3, vec![label("c002", "SETUP > AUDIO: Language B 5.1", Level::Replayed, Some(true))]),
            streams(4, vec![label("c003", "SETUP > AUDIO: COMMENTARY 1", Level::Checked, Some(true))]),
        ],
        subtitles: vec![
            streams(7, vec![label("c004", "SETUP > SUBTITLES: Language A", Level::Replayed, Some(true))]),
            streams(8, vec![label("c006", "POP-UP > SUBTITLES: Langue C — ÉTÉ", Level::Read, Some(true))]),
        ],
        subtitles_off: vec![label("c005", "SETUP > SUBTITLES: OFF", Level::Replayed, Some(false))],
        other_playlists: vec![other("01001", 1, true), other("00801", 1, false)],
        out_of_range: vec![],
        no_effect: vec!["c009".to_string()],
        roles: None,
    };
    assert_eq!(block, want);
    assert_eq!(block.audio[1].labels[0].label, "SETUP > AUDIO: Language B 5.1");
    let block = title_block(&map, &disc, title(&disc, 1), &matching());
    let want = MenuBlock {
        playlist: Some("01001".to_string()),
        starts: vec![c007],
        audio: vec![],
        subtitles: vec![],
        subtitles_off: vec![],
        other_playlists: vec![other("00800", 7, true), other("00801", 1, false)],
        ..want
    };
    assert_eq!(block, want);
    let text = serde_json::to_string(&block).unwrap();
    let keys = [
        "map", "map_level", "identity", "playlist", "starts", "audio", "subtitles", "subtitles_off", "other_playlists",
        "out_of_range", "no_effect",
    ];
    let at: Vec<usize> = keys.iter().map(|key| text.find(&format!("\"{key}\":")).unwrap()).collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
}

#[test]
fn a_title_without_a_source_file_puts_every_effect_elsewhere() {
    let (map, disc) = (fixture_map(), menu_disc());
    let mut bare = title(&disc, 0).clone();
    bare.source_file = None;
    let block = title_block(&map, &disc, &bare, &matching());
    assert_eq!(block.playlist, None);
    assert!(block.starts.is_empty() && block.audio.is_empty() && block.subtitles.is_empty());
    assert!(block.subtitles_off.is_empty() && block.out_of_range.is_empty());
    assert_eq!(block.other_playlists, [other("00800", 7, true), other("01001", 1, true), other("00801", 1, false)]);
    assert_eq!(block.no_effect, ["c009"]);
}

#[test]
fn a_play_without_an_effect_is_listed_under_no_effect() {
    let map = edited(|map| {
        map["play"]["effect"] = Value::Null;
        map["play"]["check"]["level"] = json!("read");
        map["play"]["check"]["note"] = json!("not explored");
        map["choices"][0]["differs_from_play"] = Value::Null;
    });
    let disc = menu_disc();
    let report = check(&map, &disc, matching());
    assert_eq!(report.lines[2], "play: no effect recorded");
    assert_eq!(report.lines.last().unwrap(), &check_line("ok", [10, 7, 1, 2, 0, 0]));
    let block = title_block(&map, &disc, title(&disc, 0), &matching());
    assert!(block.starts.is_empty());
    assert_eq!(block.no_effect, ["play", "c009"]);
    assert_eq!(block.audio[0].labels[0].id, "c001");
    assert_eq!(block.audio[0].labels[0].differs_from_play, None);
}

#[test]
fn an_own_kind_choice_without_a_number_labels_no_stream() {
    let map = edited(|map| {
        map["choices"][2]["effect"]["audio"] = Value::Null;
        map["choices"][3]["effect"]["subtitle"] = Value::Null;
        map["choices"][4]["effect"]["subtitle"] = Value::Null;
        map["choices"][6]["effect"]["subtitle_shown"] = json!(true);
    });
    let disc = menu_disc();
    let report = check(&map, &disc, matching());
    assert_eq!(
        report.lines[5..10],
        [
            "c003: playlist 00800, title 0: audio none, no audio stream named (warning), subtitle 1 off",
            "c004: playlist 00800, title 0: audio 1 = stream 1, subtitle none shown, no subtitle stream named (warning)",
            "c005: playlist 00800, title 0: audio 1 = stream 1, subtitle none",
            "c006: playlist 00800, title 0: audio 1 = stream 1, subtitle 3 = stream 8",
            "c007: playlist 01001, title 1: audio 1 = stream 1, subtitle none shown",
        ]
    );
    assert_eq!((report.warnings, report.failed), (2, 0));
    assert_eq!(report.lines.last().unwrap(), &check_line("ok", [10, 8, 1, 1, 2, 0]));
    let block = title_block(&map, &disc, title(&disc, 0), &matching());
    assert_eq!(block.out_of_range, ["c003", "c004"]);
    assert_eq!(ids(&block.subtitles_off), ["c005"]);
    let audio: Vec<u32> = block.audio.iter().map(|row| row.stream).collect();
    let subtitles: Vec<u32> = block.subtitles.iter().map(|row| row.stream).collect();
    assert_eq!((audio, subtitles), (vec![1, 3], vec![8]));
}

#[test]
fn a_disc_folder_is_forty_hex_digits_upper_cased() {
    let id = "0123456789abcdef0123456789ABCDEF01234567";
    assert_eq!(disc_folder(&format!(" {id}\n")), Some(id.to_uppercase()));
    for bad in ["", "0123", &id[1..], &format!("{id}0"), &id.replace('0', "g"), &id.replace('1', "/")] {
        assert_eq!(disc_folder(bad), None, "{bad:?}");
    }
}
