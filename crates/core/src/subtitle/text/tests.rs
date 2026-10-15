//! Tests of `subtitle::text`. The Python `test_subtitle_text` cases are ported one for one, in modules named after
//! its classes (`fold_and_lines`, `classes`, `rule`, `files`); the `added_*` modules are new, with expected values
//! Python 3.9.6 printed once (2026-10-13) from the same synthetic inputs. Constructed text only: made-up words, no
//! line of any real subtitle track.

use super::*;

// ------------------------------------------------------------------ FoldAndLines

mod fold_and_lines {
    use super::*;

    #[test]
    fn fold_notes_accents_and_punctuation() {
        let s = "\u{266A} La ni\u{00F1}a \u{00BF}qu\u{00E9}? \u{2019}\u{266B}";
        assert_eq!(fold(s), "# La nina que? '#");
    }

    #[test]
    fn split_joined_uses_the_line_count() {
        assert_eq!(split_joined("| almost fell. | | went home", Some(2)), ["| almost fell.", "| went home"]);
        assert_eq!(split_joined("It is fine if | go", Some(1)), ["It is fine if | go"]);
        assert_eq!(split_joined("First line | Second line", None), ["First line", "Second line"]);
    }

    #[test]
    fn corrections() {
        let mut c: Ordered<usize> = Ordered::new();
        assert_eq!(correct(&fold("| almost fell."), &mut c), "I almost fell.");
        assert_eq!(correct("lam here, |!", &mut c), "I am here, I!");
        assert_eq!(correct("(WHlSTLlNG) BORlS", &mut c), "(WHISTLING) BORIS");
        assert_eq!(correct("l'm sure", &mut c), "I'm sure");
        assert!(c.get("pipe_I").copied().unwrap_or(0) >= 2);
    }
}

// ------------------------------------------------------------------ new: the Python spellings

/// crates/core/tests/fixtures/corrections.json (Step 2): per synthetic line, each correction's re.subn result on
/// the line alone, correct()'s result and its eight counts.
#[derive(serde::Deserialize)]
struct CorrectionsFixture {
    names: Vec<String>,
    cases: Vec<(String, Vec<(String, usize)>, String, Vec<usize>)>,
}

mod added_helpers {
    use super::*;

    #[test]
    fn round_is_half_even_on_the_exact_value() {
        // (x, round(x, 3), round(x, 4)) as Python 3.9.6 printed them: ties only where x is exactly a half
        // (0.0625, 0.03125, ...), otherwise the side the binary value lies on.
        let cases: [(f64, f64, f64); 18] = [
            (0.0005, 0.001, 0.0005),
            (0.0015, 0.002, 0.0015),
            (0.0025, 0.003, 0.0025),
            (0.0035, 0.004, 0.0035),
            (0.0625, 0.062, 0.0625),
            (0.1875, 0.188, 0.1875),
            (0.3125, 0.312, 0.3125),
            (0.9375, 0.938, 0.9375),
            (0.5625, 0.562, 0.5625),
            (0.03125, 0.031, 0.0312),
            (0.09375, 0.094, 0.0938),
            (1.0 / 3.0, 0.333, 0.3333),
            (2.0 / 3.0, 0.667, 0.6667),
            (-0.0004, -0.0, -0.0004),
            (0.0004999, 0.0, 0.0005),
            (0.12345, 0.123, 0.1235),
            (0.99951, 1.0, 0.9995),
            (0.99949, 0.999, 0.9995),
        ];
        for (x, r3, r4) in cases {
            assert_eq!(py_round(x, 3).to_bits(), r3.to_bits(), "round({x}, 3)");
            assert_eq!(py_round(x, 4).to_bits(), r4.to_bits(), "round({x}, 4)");
        }
        // (x, ndigits, round(x, ndigits)) where the printed half is not the binary value's.
        let more: [(f64, i32, f64); 8] = [
            (1.0005, 3, 1.0),
            (0.0045, 3, 0.004),
            (0.1235, 3, 0.123),
            (2.675, 2, 2.67),
            (0.0155, 3, 0.015),
            (0.0155, 2, 0.02),
            (0.4445, 3, 0.445),
            (0.7005, 3, 0.701),
        ];
        for (x, ndigits, want) in more {
            assert_eq!(py_round(x, ndigits).to_bits(), want.to_bits(), "round({x}, {ndigits})");
        }
        assert!(py_round(f64::NAN, 3).is_nan());
        assert_eq!(py_round(f64::INFINITY, 4), f64::INFINITY);
    }

    #[test]
    fn islower_and_isupper_follow_python() {
        // (text, text.islower(), text.isupper()) as Python 3.9.6 printed them (Unicode 13).
        let cases: [(&str, bool, bool); 20] = [
            ("a", true, false),
            ("A", false, true),
            ("\u{00AA}", true, false),
            ("\u{02B0}", true, false),
            ("\u{2160}", false, true),
            ("\u{01C5}", false, false),
            ("1", false, false),
            ("", false, false),
            ("'", false, false),
            ("\u{00DF}", true, false),
            ("\u{03A3}", false, true),
            ("aB", false, false),
            ("a1", true, false),
            ("\u{01C5}a", false, false),
            ("A\u{01C5}", false, false),
            ("1a", true, false),
            ("1A", false, true),
            ("\u{24B6}", false, true),
            ("\u{24D0}", true, false),
            ("\u{2170}", true, false),
        ];
        for (text, lower, upper) in cases {
            assert_eq!((py_islower(text), py_isupper(text)), (lower, upper), "{text:?}");
        }
    }

    #[test]
    fn split_joined_scores_bare_pipes_as_the_python() {
        // (text, nlines, split_joined(text, nlines)) as Python 3.9.6 printed them. 'A | B |' shows that an empty
        // next token counts as "-([" does (the empty string is in every string).
        let cases: &[(&str, Option<i64>, &[&str])] = &[
            ("one | two |", Some(1), &["one | two |"]),
            ("one | two |", Some(2), &["one | two"]),
            ("one  | two", None, &["one", "two"]),
            ("|  |", Some(1), &["|  |"]),
            ("| | (yes) | -no", Some(2), &["| | (yes)", "-no"]),
            ("a | \u{00AA} | B", Some(2), &["a | \u{00AA}", "B"]),
            ("x | | | y", Some(2), &["x", "| | y"]),
            ("x | | | y", Some(0), &["x", "y"]),
            ("", None, &[]),
            ("   ", Some(3), &[]),
            ("| | |", Some(2), &["| |"]),
            ("it | 'tis | | Off", Some(2), &["it | 'tis |", "Off"]),
            ("A | B |", Some(2), &["A | B"]),
            ("A |  B", Some(2), &["A", "B"]),
            ("A | B | c", Some(2), &["A", "B | c"]),
            ("| ( x", Some(1), &["| ( x"]),
            ("x | (y", Some(1), &["x | (y"]),
        ];
        for (text, nlines, want) in cases {
            assert_eq!(split_joined(text, *nlines), *want, "split_joined({text:?}, {nlines:?})");
        }
    }

    #[test]
    fn corrections_match_the_python_on_every_fixture_line() {
        let fixture: CorrectionsFixture =
            serde_json::from_str(include_str!("../../../tests/fixtures/corrections.json")).expect("the fixture parses");
        let mut names: Vec<&str> = corrections().iter().map(|(name, _, _)| *name).collect();
        names.push(PIPE_LEFT);
        assert_eq!(fixture.names, names);
        assert_eq!(fixture.cases.len(), 592);
        for (line, each, fixed, counts) in &fixture.cases {
            assert_eq!(each.len(), 7, "{line:?}");
            for ((name, rx, rep), (want, k)) in corrections().iter().zip(each) {
                assert_eq!(subn(rx, rep, line), (want.clone(), *k), "{name} on {line:?}");
            }
            let mut got: Ordered<usize> = Ordered::new();
            assert_eq!(&correct(line, &mut got), fixed, "correct({line:?})");
            let got_counts: Vec<usize> = got.iter().map(|(_, k)| *k).collect();
            assert_eq!(&got_counts, counts, "the counts of correct({line:?})");
        }
    }

    #[test]
    fn marker_patterns_need_no_look_around_and_match_the_python() {
        // Building each pattern with the regex crate, which has no look-around, is the proof that it needs none.
        let all = [&*RE_CUE, &*RE_LABEL, &*RE_CHEV, &*RE_NOTE, &*RE_NOTE_LOOSE, &*RE_WRAP, &*RE_WORD, &*RE_WORDLIKE];
        assert!(all.iter().all(|rx| !rx.as_str().is_empty()));

        // What Python 3.9.6 printed for these made-up lines (spans are character offsets; the lines are ASCII).
        // RE_CUE.finditer: (line, [(start, end, group 1)]).
        const CUE: &[(&str, &[(usize, usize, &str)])] = &[
            ("(DOOR SLAMS) We babin.", &[(0, 12, "DOOR SLAMS")]),
            ("[sighs] (laughs)", &[(0, 7, "sighs"), (8, 16, "laughs")]),
            ("( x )", &[]),
            ("(ab)", &[(0, 4, "ab")]),
            ("(a)", &[]),
            ("[DOG BARKING]", &[(0, 13, "DOG BARKING")]),
            ("(SIREN WAILING)]", &[(0, 15, "SIREN WAILING")]),
            ("[open (close]", &[(6, 13, "close")]),
            ("(he said, \"no\")", &[]),
            ("(x\u{1c}y) (\u{1c}THUD\u{1c})", &[(6, 14, "THUD")]),
            ("(A&B: 1/2; ok?)", &[(0, 15, "A&B: 1/2; ok?")]),
            ("(9)", &[]),
            ("('tis)", &[]),
            ("((nested))", &[(1, 9, "nested")]),
            (
                "(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)",
                &[(0, 82, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")],
            ),
            ("(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)", &[]),
            ("(-ab)", &[]),
            ("(ab-)", &[]),
        ];
        // RE_LABEL.match: (line, Some((group 1, end of group 1)) or None).
        const LABEL: &[(&str, Option<(&str, usize)>)] = &[
            ("ANNA: Hello there.", Some(("ANNA", 4))),
            ("- BOB JR.: Yes.", Some(("BOB JR.", 9))),
            ("DR. KALON: We go.", Some(("DR. KALON", 9))),
            ("A: no", Some(("A", 1))),
            ("ANNA:no", None),
            ("Anna: hi", None),
            ("  - ANNA SMITH JONES: x", Some(("ANNA SMITH JONES", 20))),
            ("ANNA B C D: x", None),
            ("ANNA:\u{1c}yes", Some(("ANNA", 4))),
            ("MAN 2 : run", Some(("MAN 2", 5))),
            ("O'NEIL: hey", Some(("O'NEIL", 6))),
            ("ANNA:  ", None),
            ("-ANNA: x", Some(("ANNA", 5))),
            ("ANNA :x", None),
        ];
        // RE_CHEV.match: (line, matched).
        const CHEV: &[(&str, bool)] =
            &[(">> hello", true), ("  >>x", true), ("> x", false), ("\u{1c}>>", true), ("x >>", false)];
        // RE_NOTE.search: (line, matched).
        const NOTE: &[(&str, bool)] = &[
            ("# la la", true),
            ("la la #", true),
            ("*la*", true),
            ("la # la", false),
            ("#", true),
            ("la la #\u{1c}", true),
            ("\u{1c}* la", true),
            ("la*la", false),
        ];
        // RE_NOTE_LOOSE.search: (line, matched).
        const NOTE_LOOSE: &[(&str, bool)] = &[
            ("J la la", true),
            ("la la Jf", true),
            ("JJ", false),
            ("f x", true),
            ("x d", true),
            ("$ y", true),
            ("y %", true),
            ("xJ y", false),
            ("Jl la", true),
            ("la P", true),
            ("la P.", false),
            ("D\u{1c}la", true),
            ("la\u{1c}d", true),
            ("la  Ji  ", true),
        ];
        // RE_WRAP.match: (line, matched).
        const WRAP: &[(&str, bool)] = &[
            ("|door closes|", true),
            ("{door closes}", true),
            ("- |open it.|", true),
            ("|a|", false),
            ("|ab (x)|", false),
            ("|1door|", false),
            (" { We go! } ", true),
            ("|ab}", true),
            ("{ab|", true),
            ("|ab", false),
            ("-\u{1c}|ab|\u{1c}", true),
        ];
        // RE_WORD.findall: (line, words).
        const WORD: &[(&str, &[&str])] = &[
            ("don't stop i'm o'clock x'y 'tis", &["don't", "stop", "i'm", "o'clock", "x'y", "tis"]),
            ("abc-def gh''ij", &["abc", "def", "gh", "ij"]),
            ("we're 'twas rock'n'roll", &["we're", "twas", "rock'n", "roll"]),
        ];
        // wordlike (RE_WORDLIKE.fullmatch): (token, matched).
        const WORDLIKE: &[(&str, bool)] = &[
            ("Hello", true),
            ("\"Hello,\"", true),
            ("(well)", true),
            ("e-mail", true),
            ("rock'n'roll", true),
            ("3.14", true),
            ("12:30", true),
            ("--", false),
            ("x1", false),
            ("'", false),
            ("a--b", false),
            ("[ok]", true),
            ("#", false),
            ("a-", true),
            ("'a'", true),
            ("1,000.5", true),
            ("1.", true),
            ("a.b", false),
            ("-(a)-", true),
            ("\"'(['a'])\"!?", true),
        ];

        for (line, want) in CUE {
            let got: Vec<(usize, usize, &str)> = RE_CUE
                .captures_iter(line)
                .map(|caps| {
                    let whole = caps.get(0).expect("group 0");
                    (whole.start(), whole.end(), caps.get(1).expect("group 1").as_str())
                })
                .collect();
            assert_eq!(got, *want, "RE_CUE on {line:?}");
        }
        for (line, want) in LABEL {
            let got = RE_LABEL.captures(line).map(|caps| {
                let group = caps.get(1).expect("group 1");
                (group.as_str(), group.end())
            });
            assert_eq!(got, *want, "RE_LABEL on {line:?}");
        }
        for (rx, cases, name) in
            [(&*RE_CHEV, CHEV, "RE_CHEV"), (&*RE_NOTE, NOTE, "RE_NOTE"), (&*RE_NOTE_LOOSE, NOTE_LOOSE, "RE_NOTE_LOOSE")]
        {
            for (line, want) in cases {
                assert_eq!(rx.is_match(line), *want, "{name} on {line:?}");
            }
        }
        for (line, want) in WRAP {
            assert_eq!(RE_WRAP.is_match(line), *want, "RE_WRAP on {line:?}");
        }
        for (line, want) in WORD {
            let got: Vec<&str> = RE_WORD.find_iter(line).map(|word| word.as_str()).collect();
            assert_eq!(got, *want, "RE_WORD on {line:?}");
        }
        for (token, want) in WORDLIKE {
            assert_eq!(wordlike(token), *want, "wordlike({token:?})");
        }
    }

    #[test]
    fn stop_lists_are_the_pythons() {
        // (lang, len(STOP[lang]), ' '.join(sorted(STOP[lang]))) as Python 3.9.6 printed them.
        let want: [(&str, usize, &str); 4] = [
            (
                "en",
                60,
                "about all and are at be been can can't could did didn't do don't for from get go have her here him \
                 i i'm if in is it it's just know my not of oh okay on out right she so that that's the there they \
                 this to up we well were what will with would yes you you're your",
            ),
            (
                "es",
                47,
                "al aqui como con cuando de del donde el ella ellos en eso esta este esto hay la las lo los mas mi muy \
                 nada nos os para pero por porque puede se senor si su te tengo tiene todo un una usted vamos y ya yo",
            ),
            (
                "fr",
                36,
                "avec c'est ca ce cette dans elle est et fait ici il ils j'ai je les lui ma mais moi mon ne non nous \
                 ou oui pas pour qui rien sont suis sur toi tout vous",
            ),
            (
                "de",
                35,
                "aber auch auf bin bist das dem den der dich die ein eine er fur haben hier ich ihr ist ja kann mich \
                 mir mit nein nicht noch sein sie und wie wir wird zu",
            ),
        ];
        let langs: Vec<&str> = STOP.iter().map(|(lang, _)| *lang).collect();
        assert_eq!(langs, ["en", "es", "fr", "de"]);
        for (lang, size, words) in want {
            let set = stop(lang);
            assert_eq!(set.len(), size, "{lang}");
            assert_eq!(set.iter().copied().collect::<Vec<&str>>().join(" "), words, "{lang}");
        }
    }

    #[test]
    fn pyset_iterates_in_cpythons_order() {
        // list({a} | {b}) as Python 3.9.6 printed it: slot order, so {3} | {11} is [11, 3] (11 & 7 == 3).
        let pairs: [((i64, i64), [i64; 2]); 12] = [
            ((0, 1), [0, 1]),
            ((1, 0), [0, 1]),
            ((2, 10), [2, 10]),
            ((10, 2), [10, 2]),
            ((8, 0), [8, 0]),
            ((0, 8), [0, 8]),
            ((3, 11), [11, 3]),
            ((11, 3), [3, 11]),
            ((7, 15), [15, 7]),
            ((16, 0), [16, 0]),
            ((5, 13), [13, 5]),
            ((1, 9), [1, 9]),
        ];
        for ((a, b), want) in pairs {
            let got: Vec<i64> = PySet::singleton(a).union(&PySet::singleton(b)).iter().collect();
            assert_eq!(got, want, "{{{a}}} | {{{b}}}");
        }
        // (({3} | {11}) | {19}) | {27}
        let chain = PySet::singleton(3)
            .union(&PySet::singleton(11))
            .union(&PySet::singleton(19))
            .union(&PySet::singleton(27));
        assert_eq!(chain.iter().collect::<Vec<i64>>(), [11, 19, 3, 27]);
        // big = big | {n} from set(), eight times: two resizes on the way.
        let mut big = PySet::empty();
        for n in [40, 8, 16, 0, 24, 32, 1, 9] {
            big = big.union(&PySet::singleton(n));
        }
        assert_eq!(big.iter().collect::<Vec<i64>>(), [16, 0, 32, 1, 9, 40, 8, 24]);
        // a | a is a copy (set_or returns set_copy for one object): the same members.
        let copy = big.copy();
        assert_eq!(copy.len(), 8);
        assert!(copy.contains(40) && !copy.contains(2));
        let mut members: Vec<i64> = copy.iter().collect();
        members.sort();
        assert_eq!(members, [0, 1, 8, 9, 16, 24, 32, 40]);
    }
}
// ------------------------------------------------------------------ new: loading

mod added_loading {
    use super::*;
    use crate::recording::files::{read_jsonl, PacketsFile};

    fn rec(t: f64, pts: bool, lines: &[&str]) -> OcrRecord {
        OcrRecord { t, pts, lines: lines.iter().map(|line| line.to_string()).collect() }
    }

    fn at(starts: &[f64]) -> Vec<Event> {
        starts.iter().map(|&start| Event { start, dur: 1.0, lines: Vec::new() }).collect()
    }

    #[test]
    fn packet_events_read_ffprobe_packets_as_the_python() {
        // ffprobe's strings, a number where a string was expected, "N/A" and "" and null durations, a packet
        // without pts_time, one at the threshold, and a stable sort of equal starts.
        let packets: PacketsFile = serde_json::from_str(
            r#"{"packets": [
                {"pts_time": "5.000", "duration_time": "1.000", "size": "100"},
                {"pts_time": "1.500", "duration_time": "N/A", "size": "65"},
                {"pts_time": "1.500", "duration_time": "", "size": 200},
                {"duration_time": "1.000", "size": "900"},
                {"pts_time": "0.250", "size": "64"},
                {"pts_time": 3, "duration_time": 0.5, "size": 4000.0},
                {"pts_time": "2.000", "duration_time": null}
            ]}"#,
        )
        .unwrap();
        let got: Vec<(f64, f64)> =
            packet_events(packets.packets(), 64).unwrap().iter().map(|e| (e.start, e.dur)).collect();
        // [{'start': 1.5, 'dur': 0.0}, {'start': 1.5, 'dur': 0.0}, {'start': 3.0, 'dur': 0.5}, {'start': 5.0, 'dur': 1.0}]
        assert_eq!(got, [(1.5, 0.0), (1.5, 0.0), (3.0, 0.5), (5.0, 1.0)]);
        // A bare list, and clear_max_bytes 0 (a text stream): a packet without a size is size 0 and not an event.
        let list: PacketsFile = serde_json::from_str(r#"[{"pts_time": "2.000"}, {"pts_time": "1.000", "size": "1"}]"#).unwrap();
        let got: Vec<(f64, f64)> = packet_events(list.packets(), 0).unwrap().iter().map(|e| (e.start, e.dur)).collect();
        assert_eq!(got, [(1.0, 0.0)]);
        // int() and float() refusals carry the Python's text.
        let bad: PacketsFile = serde_json::from_str(r#"[{"pts_time": "x", "size": "4000"}]"#).unwrap();
        assert_eq!(packet_events(bad.packets(), 64), Err("could not convert string to float: 'x'".to_string()));
        let bad: PacketsFile = serde_json::from_str(r#"[{"pts_time": "1", "size": "4k"}]"#).unwrap();
        assert_eq!(packet_events(bad.packets(), 64), Err("invalid literal for int() with base 10: '4k'".to_string()));
    }

    #[test]
    fn ocr_records_take_lines_or_split_text() {
        // The records and frames.json the Python read; ocr_records printed
        // [{'t': 1.0, 'pts': False, 'lines': ['| went', '| home']}, {'t': 7.5, 'pts': True, 'lines': ['a', '3']},
        //  {'t': 0.0, 'pts': False, 'lines': ['b', 'c']}, {'t': 4.0, 'pts': False, 'lines': ['x', 'y']}].
        let jsonl = concat!(
            "{\"frame\": 0, \"t\": 0.0, \"text\": \"\"}\n",
            "{\"frame\": 1, \"t\": 1.0, \"text\": \"| went | | home\"}\n",
            "{\"frame\": 2, \"pts\": 7.5, \"t\": 2.0, \"lines\": [\" a \", \"\", 3, \"  \"]}\n",
            "{\"frame\": 3, \"lines\": \"not a list\", \"text\": \" b | c \"}\n",
            "{\"t\": 4.0, \"text\": \"x | y\"}\n",
            "{\"frame\": 5, \"lines\": []}\n",
            "\n",
        );
        let records = read_jsonl(jsonl).unwrap();
        let counts: BTreeMap<i64, Option<i64>> = BTreeMap::from([(1, Some(2)), (3, None)]);
        let recs = ocr_records(&records, &counts).unwrap();
        assert_eq!(
            recs,
            [
                rec(1.0, false, &["| went", "| home"]),
                rec(7.5, true, &["a", "3"]),
                rec(0.0, false, &["b", "c"]),
                rec(4.0, false, &["x", "y"]),
            ]
        );
    }

    #[test]
    fn pair_text_pairs_by_time_with_the_pythons_ties() {
        // Equal counts: by order.
        let mut events = at(&[1.0, 2.0]);
        let recs = [rec(9.0, false, &["p"]), rec(0.0, false, &["q"])];
        assert_eq!(pair_text(&mut events, &recs), (PairMethod::Order, 0));
        assert_eq!(events[0].lines, ["p"]);
        // By time from the first frame ('t' plus the first event's start): ('time', 1) and
        // [['a', 'b'], ['c'], [], ['e']] (b is as near to 10.0 as to 10.4 in decimal, nearer 10.0 in binary).
        let mut events = at(&[10.0, 10.4, 11.0, 20.0]);
        let recs = [
            rec(0.0, false, &["a"]),
            rec(0.2, false, &["b"]),
            rec(0.65, false, &["c"]),
            rec(5.0, false, &["d"]),
            rec(9.8, false, &["e"]),
        ];
        assert_eq!(pair_text(&mut events, &recs), (PairMethod::Time, 1));
        let lines: Vec<Vec<String>> = events.iter().map(|e| e.lines.clone()).collect();
        assert_eq!(lines, [vec!["a", "b"], vec!["c"], vec![], vec!["e"]]);
        // Packet times ('pts'): no offset; ('time', 2) and [['tie'], []].
        let mut events = at(&[10.0, 10.4]);
        let recs = [rec(10.2, true, &["tie"]), rec(10.71, true, &["late"]), rec(9.6999, true, &["early"])];
        assert_eq!(pair_text(&mut events, &recs), (PairMethod::Time, 2));
        let lines: Vec<Vec<String>> = events.iter().map(|e| e.lines.clone()).collect();
        assert_eq!(lines, [vec!["tie"], vec![]]);
        // No events: every frame is left; no frames for one event: ('time', 0).
        assert_eq!(pair_text(&mut [], &recs), (PairMethod::Time, 3));
        assert_eq!(pair_text(&mut at(&[1.0]), &[]), (PairMethod::Time, 0));
    }
}
// ------------------------------------------------------------------ the generators of test_subtitle_text

use crate::testsupport::PyRandom;

/// Made-up words (letters only), so that no real subtitle text is needed. Py: made_up.
fn made_up(consonants: &str, vowels: &str, count: usize) -> Vec<String> {
    let mut words = Vec::new();
    for a in consonants.chars() {
        for v in vowels.chars() {
            for b in consonants.chars() {
                for w in vowels.chars() {
                    words.push(format!("{a}{v}{b}{w}n"));
                }
            }
        }
    }
    words.truncate(count);
    words
}

static NOUNS: LazyLock<Vec<String>> = LazyLock::new(|| made_up("bdgkl", "aeiou", 400));
static VERBS: LazyLock<Vec<String>> = LazyLock::new(|| made_up("mprst", "aeiou", 200));
static OTHER: LazyLock<Vec<String>> = LazyLock::new(|| made_up("fhvwz", "aeiou", 400));
static OTHER_VERBS: LazyLock<Vec<String>> = LazyLock::new(|| made_up("cjnqx", "aeiou", 200));
/// Py: SPANISH (a made-up sentence of common Spanish words).
const SPANISH: &str = "el perro de la casa y los ninos que no tienen nada para comer pero ella dice que esta bien con \
                       su madre aqui en el pueblo cuando llueve";

fn spanish() -> Vec<&'static str> {
    SPANISH.split_whitespace().collect()
}

/// Which word lists a sentence draws from: NOUNS with VERBS, or OTHER with OTHER_VERBS. Py: `nouns is NOUNS`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Words {
    Nouns,
    Other,
}

/// Py: sentence(rnd, nouns), its four choices in the Python's order.
fn sentence(rnd: &mut PyRandom, nouns: Words) -> String {
    let (nouns, verbs): (&[String], &[String]) = match nouns {
        Words::Nouns => (NOUNS.as_slice(), VERBS.as_slice()),
        Words::Other => (OTHER.as_slice(), OTHER_VERBS.as_slice()),
    };
    let verb = rnd.choice(verbs).clone();
    let a = rnd.choice(nouns).clone();
    let b = rnd.choice(nouns).clone();
    let c = rnd.choice(nouns).clone();
    format!("we {verb} the {a} and the {b} with the {c}")
}

/// str.capitalize on ASCII: the first character upper case, the rest lower case.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

/// Py: track(n, events, tag='eng', forced=False).
fn track(n: i64, events: Vec<Event>, tag: &str, forced: bool) -> Track {
    Track::new(n, tag, None, forced, events)
}

/// Py: track(n, events) (an English stream, not flagged forced).
fn eng(n: i64, events: Vec<Event>) -> Track {
    track(n, events, "eng", false)
}

/// Py: dialogue(count, seed, nouns, start, step).
fn dialogue(count: usize, seed: u64, nouns: Words, start: f64, step: f64) -> Vec<Event> {
    let mut rnd = PyRandom::new(seed);
    (0..count)
        .map(|i| Event {
            start: start + i as f64 * step,
            dur: 2.5,
            lines: vec![capitalize(&sentence(&mut rnd, nouns)) + "."],
        })
        .collect()
}

/// Py: dialogue() with its defaults (120 events, seed 1, NOUNS, from 10.0 s every 4.0 s).
fn base() -> Vec<Event> {
    dialogue(120, 1, Words::Nouns, 10.0, 4.0)
}

/// list.sort(key=lambda e: e['start']): stable.
fn sort_by_start(events: &mut [Event]) {
    events.sort_by(|a, b| py_cmp(a.start, b.start));
}

/// Adds stand-alone sound cues between events and capital speaker labels to some events. Py: sdh_version(events)
/// with its defaults (8 cues, 6 labels).
fn sdh_version(events: &[Event]) -> Vec<Event> {
    let (cues, labels) = (8usize, 6usize);
    let mut out: Vec<Event> = events.to_vec();
    let step = events.len() / (cues.max(labels) + 1);
    for k in 0..labels {
        let e = &mut out[2 + step * k];
        e.lines[0] = format!("ANNA: {}", e.lines[0]);
    }
    let sounds = [
        "(DOOR SLAMS)",
        "(PHONE RINGING)",
        "(GLASS SHATTERS)",
        "[sighs]",
        "(SIREN WAILING)",
        "(KNOCKING)",
        "(DOG BARKING)",
        "(WIND HOWLING)",
        "(COUGHS)",
        "(THUNDER RUMBLING)",
    ];
    for k in 0..cues {
        let e = &events[3 + step * k];
        out.push(Event { start: e.start + 2.8, dur: 0.9, lines: vec![sounds[k % sounds.len()].to_string()] });
    }
    sort_by_start(&mut out);
    out
}

/// Py: measured(*tracks).
fn measured(tracks: Vec<Track>) -> Vec<Track> {
    measured_with(&Rule::default(), tracks)
}

/// measured under a changed rule (the Python's mock.patch.dict(st.RULE, ...)).
fn measured_with(rule: &Rule, mut tracks: Vec<Track>) -> Vec<Track> {
    for t in tracks.iter_mut() {
        measure(t, rule);
    }
    tracks
}

/// The characters of OCR junk. Py: 'xq#%}{3|'.
const JUNK: [char; 8] = ['x', 'q', '#', '%', '}', '{', '3', '|'];

/// Events of OCR junk: an unreadable track. Py: junk(count, seed, start, step).
fn junk(count: usize, seed: u64, start: f64, step: f64) -> Vec<Event> {
    let mut rnd = PyRandom::new(seed);
    (0..count)
        .map(|i| {
            let line: String = (0..12).map(|_| *rnd.choice(&JUNK)).collect();
            Event { start: start + i as f64 * step, dur: 0.8, lines: vec![line] }
        })
        .collect()
}

/// Readable made-up text with one English stopword in nine words: its language is 'unknown' (T1b).
/// Py: unclear(count, seed, start, step).
fn unclear(count: usize, seed: u64, start: f64, step: f64) -> Vec<Event> {
    let mut rnd = PyRandom::new(seed);
    (0..count)
        .map(|i| {
            let words: Vec<String> = (0..8).map(|_| rnd.choice(OTHER.as_slice()).clone()).collect();
            Event { start: start + i as f64 * step, dur: 2.5, lines: vec![capitalize(&format!("the {}", words.join(" "))) + "."] }
        })
        .collect()
}

/// The OCR junk garbled() appends to every line.
const GARBLE: &str = " x#%}{ q|}{# 3%}{x }{#q";

/// The same events with OCR junk after every line: the words stay, but the track is unreadable. Py: garbled.
fn garbled(events: &[Event]) -> Vec<Event> {
    events
        .iter()
        .map(|e| Event { lines: e.lines.iter().map(|line| line.clone() + GARBLE).collect(), ..e.clone() })
        .collect()
}

/// Another language's stream without OCR text: its packets only (T4's timing). Py: unread(n, events, tag).
fn unread(n: i64, events: &[Event], tag: &str) -> Track {
    let events = events.iter().map(|e| Event { start: e.start, dur: e.dur, lines: Vec::new() }).collect();
    let mut t = Track::new(n, tag, None, false, events);
    t.text = Some(false);
    t
}

/// One event per three words of the list, no word used twice: two tracks share exactly the bigrams of the events
/// they share, so a track's share of another's words is its share of events. Py: distinct(words, start, step).
fn distinct(words: &[String], start: f64, step: f64) -> Vec<Event> {
    (0..words.len() / 3)
        .map(|i| {
            let line = format!("we {} the {} and the {}", words[3 * i], words[3 * i + 1], words[3 * i + 2]);
            Event { start: start + i as f64 * step, dur: 2.5, lines: vec![capitalize(&line) + "."] }
        })
        .collect()
}

/// Spanish lines as the tests build them: `count` events from `start` every `step` s, eight words each, capitalised,
/// drawn from Random(2). Py: the comprehension of test_language and its copies.
fn spanish_events(count: usize, start: f64, step: f64) -> Vec<Event> {
    let mut rnd = PyRandom::new(2);
    let words = spanish();
    (0..count)
        .map(|i| {
            let line: Vec<&str> = (0..8).map(|_| *rnd.choice(&words)).collect();
            Event { start: start + step * i as f64, dur: 2.0, lines: vec![capitalize(&line.join(" "))] }
        })
        .collect()
}

/// A decision as the Python's dicts write it.
fn dec(role: Role, review: Option<&str>, note: Option<&str>) -> Decision {
    Decision { role, review: review.map(str::to_string), note: note.map(str::to_string) }
}

/// st.decide under `rule`, checking the owner's 'ambiguous = keep': no track is dropped with a review.
/// Py: Rule.decide.
fn checked_decide_with(rule: &Rule, tracks: &[Track], orig_lang: &str) -> BTreeMap<i64, Decision> {
    let v = decide(tracks, Some(orig_lang), rule);
    for (n, r) in &v {
        assert!(!(r.role == Role::Drop && r.review.is_some()), "s{n} is dropped with a review");
    }
    v
}

/// Py: Rule.decide with the default RULE.
fn checked_decide(tracks: &[Track], orig_lang: &str) -> BTreeMap<i64, Decision> {
    checked_decide_with(&Rule::default(), tracks, orig_lang)
}

/// The same events `delta` seconds later. Py: [dict(e, start=e['start'] + delta) for e in events].
fn later(events: &[Event], delta: f64) -> Vec<Event> {
    events.iter().map(|e| Event { start: e.start + delta, ..e.clone() }).collect()
}

// ------------------------------------------------------------------ Classes

mod classes {
    use super::*;

    #[test]
    fn sdh_track() {
        let tracks = measured(vec![eng(0, sdh_version(&base()))]);
        let m = measures_of(&tracks[0]);
        assert_eq!(m.class, Class::Sdh);
        assert_eq!(m.SO, 8);
        assert_eq!(m.LABEL, 6);
        assert_eq!(m.lang, "en");
    }

    #[test]
    fn regular_track() {
        let tracks = measured(vec![eng(0, base())]);
        let m = measures_of(&tracks[0]);
        assert_eq!(m.class, Class::Regular);
        assert_eq!(m.M, 0);
    }

    #[test]
    fn inline_asides_are_not_sdh() {
        let mut ev = base();
        for k in 0..9 {
            ev[4 + 10 * k].lines[0].push_str(" (FBI)");
        }
        let tracks = measured(vec![eng(0, ev)]);
        let m = measures_of(&tracks[0]);
        assert_eq!(m.M, 9);
        assert_eq!(m.SO, 0);
        assert_eq!(m.class, Class::Unclear);
    }

    #[test]
    fn bracketed_notes_without_sounds_are_not_sdh() {
        let mut ev = base();
        for k in 0..8 {
            let start = ev[3 + 12 * k].start + 2.8;
            ev.push(Event { start, dur: 0.9, lines: vec!["[SIGN]".to_string()] });
        }
        sort_by_start(&mut ev);
        let tracks = measured(vec![eng(0, ev)]);
        let m = measures_of(&tracks[0]);
        assert_eq!(m.SO, 8);
        assert_eq!(m.CS, 0);
        assert_eq!(m.class, Class::Unclear);
    }

    #[test]
    fn capital_sound_lines_without_brackets_are_unclear() {
        let mut ev = base();
        for k in 0..5 {
            let start = ev[3 + 12 * k].start + 2.8;
            ev.push(Event { start, dur: 0.9, lines: vec!["DOOR SLAMS".to_string()] });
        }
        sort_by_start(&mut ev);
        let tracks = measured(vec![eng(0, ev)]);
        let m = measures_of(&tracks[0]);
        assert_eq!(m.CAPS_CUE, 5);
        assert_eq!(m.class, Class::Unclear);
    }

    #[test]
    fn all_capital_track_keeps_bracketed_cues_only() {
        let mut ev = sdh_version(&base());
        for e in ev.iter_mut() {
            e.lines = e.lines.iter().map(|line| line.to_uppercase()).collect();
        }
        let tracks = measured(vec![eng(0, ev)]);
        let m = measures_of(&tracks[0]);
        assert!(m.caps_style);
        assert_eq!(m.LABEL, 0);
        assert_eq!(m.class, Class::Sdh);
        let upper: Vec<Event> = base()
            .into_iter()
            .map(|e| Event { lines: e.lines.iter().map(|line| line.to_uppercase()).collect(), ..e })
            .collect();
        let r = measured(vec![eng(0, upper)]);
        assert_eq!(measures_of(&r[0]).class, Class::Regular);
    }

    #[test]
    fn damaged_brackets_are_unclear() {
        let mut ev = base();
        for k in 0..4 {
            let start = ev[3 + 12 * k].start + 2.8;
            ev.push(Event { start, dur: 0.9, lines: vec!["|door closes|".to_string()] });
        }
        sort_by_start(&mut ev);
        let tracks = measured(vec![eng(0, ev)]);
        let m = measures_of(&tracks[0]);
        assert_eq!(m.ODD, 4);
        assert_eq!(m.class, Class::Unclear);
    }

    #[test]
    fn note_stand_ins_do_not_make_a_track_unreadable() {
        let mut ev = sdh_version(&base());
        for e in ev.iter_mut().step_by(2) {
            e.lines = e.lines.iter().map(|line| format!("# {line} #")).collect();
        }
        let tracks = measured(vec![eng(0, ev)]);
        let m = measures_of(&tracks[0]);
        assert!(m.readable);
        assert_eq!(m.class, Class::Sdh);
        assert!(m.NOTE > 50, "{}", m.NOTE);
    }

    #[test]
    fn note_only_events_do_not_make_a_track_unreadable() {
        let mut ev = sdh_version(&base());
        // gaps = [e['start'] + 3.3 for e in ev[5:115:5]]
        let gaps: Vec<f64> = ev[5..115].iter().step_by(5).map(|e| e.start + 3.3).collect();
        for (k, start) in gaps.iter().enumerate() {
            let line = if k % 2 == 1 { "#" } else { "J" };
            ev.push(Event { start: *start, dur: 0.5, lines: vec![line.to_string()] });
        }
        sort_by_start(&mut ev);
        let tracks = measured(vec![eng(0, ev)]);
        let m = measures_of(&tracks[0]);
        assert_eq!(m.note_only, 22);
        assert_eq!(m.events, 150);
        assert!(m.readable);
        assert_eq!(m.class, Class::Sdh);
    }

    #[test]
    fn garbage_is_unreadable() {
        let tracks = measured(vec![eng(0, junk(60, 3, 10.0, 1.0))]);
        assert_eq!(measures_of(&tracks[0]).class, Class::Unreadable);
        // T0: an unreadable track is always reviewed, also a flagged forced track with no dialogue track beside it.
        let mut forced = tracks[0].clone();
        forced.forced = true;
        let v = decide(&[forced], Some("en"), &Rule::default());
        assert_eq!(v[&0].review.as_deref(), Some(AMBIGUOUS));
    }

    #[test]
    fn language() {
        let tracks = measured(vec![track(0, spanish_events(80, 10.0, 3.0), "spa", false)]);
        assert_eq!(measures_of(&tracks[0]).lang, "es");
        let e = measured(vec![eng(0, base())]);
        assert_eq!(measures_of(&e[0]).lang, "en");
    }
}

// ------------------------------------------------------------------ Rule

mod rule {
    use super::*;

    #[test]
    fn single_tracks() {
        let tracks = measured(vec![eng(0, sdh_version(&base()))]);
        assert_eq!(checked_decide(&tracks, "en")[&0].role, Role::Sdh);
        let tracks = measured(vec![eng(0, base())]);
        assert_eq!(checked_decide(&tracks, "en")[&0], dec(Role::Regular, None, None));
    }

    #[test]
    fn sdh_and_regular_by_text_not_by_count() {
        let b = base();
        let mut split = Vec::new();
        for e in &b {
            split.push(Event { dur: 1.2, ..e.clone() });
            split.push(Event { start: e.start + 1.3, dur: 1.2, ..e.clone() });
        }
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, split)]);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&0].role, v[&1].role), (Role::Sdh, Role::Regular));
        assert_eq!(v[&0].review, None);
        assert_eq!(
            (v[&0].note.as_deref(), v[&1].note.as_deref()),
            (Some("subtitle_count_disagrees"), Some("subtitle_count_disagrees"))
        );
        assert_eq!(spec87(&tracks, &Rule::default())[&1], "sdh");
    }

    #[test]
    fn retimed_regular_stays_in_the_dialogue_group() {
        let b = base();
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, later(&b, 10.0))]);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&0].role, v[&1].role), (Role::Sdh, Role::Regular));
    }

    #[test]
    fn two_regular_tracks_go_to_review() {
        let tracks = measured(vec![eng(0, base()), eng(1, later(&base(), 0.1))]);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&0].review.as_deref(), v[&0].note.as_deref()), (Some(AMBIGUOUS), Some("two regular tracks")));
        assert_eq!(v[&1].review.as_deref(), Some(AMBIGUOUS));
    }

    #[test]
    fn two_sdh_tracks_go_to_review() {
        let ev = sdh_version(&base());
        let tracks = measured(vec![eng(0, ev.clone()), eng(1, later(&ev, 0.1))]);
        let v = checked_decide(&tracks, "en");
        for n in [0, 1] {
            assert_eq!(v[&n], dec(Role::Sdh, Some(AMBIGUOUS), Some("two SDH tracks")));
        }
    }

    #[test]
    fn unclear_class_is_reviewed_with_suggested_roles() {
        fn asides() -> Vec<Event> {
            let mut ev = base();
            for k in 0..9 {
                ev[4 + 10 * k].lines[0].push_str(" (FBI)");
            }
            ev
        }
        let why = "some SDH markers (9 events, 0 sound-only, 0 naming a sound)";
        let tracks = measured(vec![eng(0, asides())]);
        assert_eq!(measures_of(&tracks[0]).class, Class::Unclear);
        assert_eq!(checked_decide(&tracks, "en")[&0], dec(Role::Regular, Some(AMBIGUOUS), Some(why)));
        let tracks = measured(vec![eng(0, sdh_version(&base())), eng(1, asides())]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&0], dec(Role::Sdh, Some(AMBIGUOUS), Some(why)));
        assert_eq!(v[&1], dec(Role::Regular, Some(AMBIGUOUS), Some(why)));
        let tracks = measured(vec![eng(0, base()), eng(1, asides())]);
        let v = checked_decide(&tracks, "en");
        for n in [0, 1] {
            assert_eq!(v[&n], dec(Role::Keep, Some(AMBIGUOUS), Some(why)));
        }
    }

    #[test]
    fn capital_only_events_in_one_track_name_the_review() {
        let b = base();
        let mut caps = b.clone();
        for e in b[10..50].iter().step_by(10) {
            caps.push(Event { start: e.start + 2.9, dur: 0.4, lines: vec!["KALON BIDUN".to_string()] });
        }
        sort_by_start(&mut caps);
        let tracks = measured(vec![eng(0, b), eng(1, caps)]);
        assert_eq!(measures_of(&tracks[1]).class, Class::Regular);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&1].note.as_deref(), Some("capital-only events in one track only (UK-style SDH?)"));
        assert_eq!(v[&0].review.as_deref(), Some(AMBIGUOUS));
    }

    #[test]
    fn an_unreadable_pair_falls_back_to_the_event_count() {
        let b = base();
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, garbled(&b))]);
        assert_eq!(measures_of(&tracks[1]).class, Class::Unreadable);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&0].role, v[&0].review.as_deref()), (Role::Sdh, Some(AMBIGUOUS)));
        assert!(v[&0].note.as_deref().unwrap_or("").starts_with("subtitle_text_unreadable"));
        assert_eq!((v[&1].role, v[&1].review.as_deref()), (Role::Regular, Some(AMBIGUOUS)));
    }

    #[test]
    fn more_than_two_tracks_go_to_review() {
        let b = base();
        let tracks = measured((0..3).map(|n| eng(n, later(&b, 0.05 * n as f64))).collect());
        let v = checked_decide(&tracks, "en");
        for n in 0..3 {
            assert_eq!(v[&n], dec(Role::Keep, Some(AMBIGUOUS), Some("more than two tracks in the dialogue group")));
        }
    }

    #[test]
    fn forced_subset_with_many_events() {
        let b = base();
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, b[40..70].to_vec())]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&1].role, Role::Forced);
        assert_eq!(v[&0].role, Role::Sdh);
    }

    #[test]
    fn forced_candidates_in_8_7_order_before_subsets() {
        let b = base();
        let tracks = measured(vec![eng(0, b.clone()), eng(1, b[0..36].to_vec()), eng(2, b[60..70].to_vec())]);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&2].role, v[&2].review.as_deref()), (Role::Forced, None));
        assert_eq!(v[&1], dec(Role::Keep, Some(AMBIGUOUS), Some("a second forced candidate")));
    }

    #[test]
    fn second_confirmed_forced_candidate_is_kept_for_review() {
        let b = base();
        let tracks = measured(vec![eng(0, b.clone()), eng(1, b[40..50].to_vec()), eng(2, b[60..70].to_vec())]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&1], dec(Role::Forced, None, Some("words found in the dialogue track")));
        assert_eq!(v[&2], dec(Role::Keep, Some(AMBIGUOUS), Some("a second forced candidate")));
    }

    #[test]
    fn forced_candidate_that_looks_like_lyrics_is_kept() {
        let b = base();
        let song: Vec<Event> = b[40..50]
            .iter()
            .map(|e| Event { lines: e.lines.iter().map(|line| format!("# {line} #")).collect(), ..e.clone() })
            .collect();
        let tracks = measured(vec![eng(0, b), eng(1, song)]);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&1].role, v[&1].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
        assert!(v[&1].note.as_deref().unwrap_or("").starts_with("forced candidate looks like lyrics"));
    }

    #[test]
    fn forced_candidate_that_partly_matches_is_kept() {
        let b = base();
        let mut half = b[40..45].to_vec();
        half.extend(dialogue(5, 7, Words::Other, 300.0, 4.0));
        let tracks = measured(vec![eng(0, b), eng(1, half)]);
        let rule = Rule::default();
        let x = contained(&tracks[1], &tracks[0], &rule).expect("enough bigrams to compare");
        assert!(rule.CB_DIFF < x && x < rule.FORCED_CB, "{x}");
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&1], dec(Role::Keep, Some(AMBIGUOUS), Some("forced candidate partly matches the dialogue track")));
    }

    #[test]
    fn forced_candidate_in_another_language_is_kept() {
        let tracks = measured(vec![eng(0, base()), track(1, spanish_events(10, 30.0, 7.0), "eng", true)]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&1], dec(Role::Keep, Some(AMBIGUOUS), Some("tagged eng, text reads es")));
        assert_eq!(spec87(&tracks, &Rule::default())[&1], "forced");
    }

    #[test]
    fn unrelated_short_track_is_not_forced() {
        let other = dialogue(11, 9, Words::Other, 13.0, 40.0);
        let tracks = measured(vec![eng(0, base()), eng(1, other)]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&1].role, Role::Keep);
        assert_eq!(v[&1].review.as_deref(), Some(AMBIGUOUS));
        assert_eq!(spec87(&tracks, &Rule::default())[&1], "forced");
    }

    #[test]
    fn commentary_dropped_only_when_words_and_timing_differ() {
        let b = base();
        let comm = dialogue(100, 5, Words::Other, 11.7, 4.0);
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, b.clone()), eng(2, comm.clone())]);
        assert_eq!(checked_decide(&tracks, "en")[&2].role, Role::Drop);
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, b.clone()), eng(2, comm)]);
        let v = checked_decide(&tracks, "es");
        assert_eq!((v[&2].role, v[&2].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
        let near: Vec<Event> = dialogue(100, 5, Words::Other, 10.0, 4.0)
            .into_iter()
            .enumerate()
            .map(|(i, e)| Event { start: e.start + if i % 2 == 1 { 0.1 } else { 0.6 }, ..e })
            .collect();
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, b.clone()), eng(2, near)]);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&2].role, v[&2].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
    }

    /// T6's drop guards, one test each: the track meets every other T6 condition (outside the dialogue group, other
    /// words and other timing than every member, readable English, original language English) and fails just one.
    /// Py: assert_t6_fixture.
    fn assert_t6_fixture(rule: &Rule, tracks: &[Track], n: usize, dialogue_ns: &[usize]) {
        let t = &tracks[n];
        assert!(measures_of(t).readable);
        assert_eq!(measures_of(t).lang, "en");
        for &k in dialogue_ns {
            let p = pair(t, &tracks[k], rule);
            assert_eq!((p.content, p.timing), (Content::Different, Timing::Diff), "s{n} against s{k}");
        }
    }

    #[test]
    fn t6_keeps_a_track_with_sound_only_cues() {
        let mut comm = dialogue(100, 5, Words::Other, 11.7, 4.0);
        let cues: Vec<Event> = (0..2)
            .map(|k| Event { start: comm[10 + 30 * k].start + 2.8, dur: 0.9, lines: vec!["(DOOR SLAMS)".to_string()] })
            .collect();
        comm.extend(cues);
        sort_by_start(&mut comm);
        let b = base();
        let rule = Rule::default();
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, b.clone()), eng(2, comm)]);
        assert_t6_fixture(&rule, &tracks, 2, &[0, 1]);
        assert_eq!(measures_of(&tracks[2]).SO, 2);
        assert_ne!(measures_of(&tracks[2]).class, Class::Sdh);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&0].role, v[&1].role), (Role::Sdh, Role::Regular));
        assert_eq!((v[&2].role, v[&2].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
    }

    #[test]
    fn t6_keeps_an_sdh_track() {
        // With today's thresholds an SDH track always has sound-only events (SDH_SO); retuned to 0, the class guard
        // alone must keep a track classed SDH whose cues all sit beside dialogue.
        let b = base();
        let comm: Vec<Event> = dialogue(100, 5, Words::Other, 11.7, 4.0)
            .into_iter()
            .enumerate()
            .map(|(i, e)| if i % 12 == 3 { Event { lines: vec![format!("{} (DOOR SLAMS)", e.lines[0])], ..e } } else { e })
            .collect();
        let rule = Rule { SDH_SO: 0, ..Rule::default() };
        let tracks = measured_with(&rule, vec![eng(0, sdh_version(&b)), eng(1, b.clone()), eng(2, comm)]);
        assert_t6_fixture(&rule, &tracks, 2, &[0, 1]);
        assert_eq!(measures_of(&tracks[2]).SO, 0);
        assert_eq!(measures_of(&tracks[2]).class, Class::Sdh);
        let v = checked_decide_with(&rule, &tracks, "en");
        assert_eq!((v[&0].role, v[&1].role), (Role::Sdh, Role::Regular));
        assert_eq!((v[&2].role, v[&2].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
    }

    #[test]
    fn t6_keeps_a_track_of_a_two_track_group() {
        let b = base();
        let comm = dialogue(100, 5, Words::Other, 11.7, 4.0);
        let rule = Rule::default();
        let tracks = measured(vec![
            eng(0, sdh_version(&b)),
            eng(1, b.clone()),
            eng(2, later(&b, 0.05)),
            eng(3, comm.clone()),
            eng(4, later(&comm, 0.1)),
        ]);
        for n in [3, 4] {
            assert_t6_fixture(&rule, &tracks, n, &[0, 1, 2]);
            assert_eq!(measures_of(&tracks[n]).SO, 0);
            assert_ne!(measures_of(&tracks[n]).class, Class::Sdh);
        }
        assert_eq!(pair(&tracks[3], &tracks[4], &rule).content, Content::Same);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&0].note.as_deref(), Some("more than two tracks in the dialogue group"));
        for n in [3, 4] {
            assert_eq!((v[&n].role, v[&n].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
        }
    }

    /// The track of `order` that stands for letter `k`.
    fn by<'a>(tracks: &'a [Track], order: &str, k: char) -> &'a Track {
        &tracks[order.find(k).expect("a letter of the order")]
    }

    #[test]
    fn one_way_chain_of_full_tracks_gives_the_same_verdicts_in_any_numbering() {
        // T2 on three full tracks: A's words in B, B's words in C, A's words not in C. A and B are both forced
        // candidates, whatever their stream numbers (T2 compares with the full tracks as they were before it).
        let words: Vec<String> =
            NOUNS.iter().chain(VERBS.iter()).chain(OTHER.iter()).chain(OTHER_VERBS.iter()).cloned().collect();
        let c = distinct(&words[..600], 10.0, 4.0);
        let q = distinct(&words[600..720], 11.7, 4.0);
        let mut b: Vec<Event> = c[..90].iter().chain(q.iter()).cloned().collect();
        sort_by_start(&mut b);
        let mut a: Vec<Event> = c[85..90].iter().chain(q.iter()).cloned().collect();
        sort_by_start(&mut a);
        let events = |k: char| -> Vec<Event> {
            match k {
                'A' => a.clone(),
                'B' => b.clone(),
                _ => c.clone(),
            }
        };
        let rule = Rule::default();
        let mut verdicts: BTreeMap<&str, BTreeMap<char, Decision>> = BTreeMap::new();
        for order in ["ABC", "BAC"] {
            let tracks = measured(order.chars().enumerate().map(|(n, k)| eng(n as i64, events(k))).collect());
            let (ta, tb, tc) = (by(&tracks, order, 'A'), by(&tracks, order, 'B'), by(&tracks, order, 'C'));
            assert!(subset_of(ta, tb, pair(ta, tb, &rule).cb, &rule));
            assert!(subset_of(tb, tc, pair(tb, tc, &rule).cb, &rule));
            assert!(!subset_of(ta, tc, pair(ta, tc, &rule).cb, &rule));
            let v = checked_decide(&tracks, "en");
            verdicts.insert(order, order.chars().enumerate().map(|(n, k)| (k, v[&(n as i64)].clone())).collect());
        }
        assert_eq!(verdicts["ABC"], verdicts["BAC"]);
        let expected: BTreeMap<char, Decision> = BTreeMap::from([
            ('C', dec(Role::Regular, None, None)),
            ('B', dec(Role::Forced, None, Some("words found in the dialogue track"))),
            ('A', dec(Role::Keep, Some(AMBIGUOUS), Some("forced candidate shares no words with the dialogue track"))),
        ]);
        assert_eq!(verdicts["ABC"], expected);
    }

    #[test]
    fn unreadable_track_outside_the_dialogue_group_is_kept() {
        let b = base();
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, b.clone()), eng(2, junk(60, 3, 11.7, 4.0))]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(
            v[&2],
            dec(Role::Keep, Some(AMBIGUOUS), Some("subtitle_text_unreadable (8.7 would drop it by timing)"))
        );
        assert_eq!(spec87(&tracks, &Rule::default())[&2], "drop");
    }

    #[test]
    fn other_language_track_picks_the_dialogue_group() {
        let sdh = sdh_version(&base());
        let comm = dialogue(100, 5, Words::Other, 11.7, 4.0);
        let mut tracks = measured(vec![eng(0, sdh.clone()), eng(1, comm.clone())]);
        tracks.push(unread(2, &sdh, "fre"));
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&0], dec(Role::Sdh, None, None));
        assert_eq!(v[&1].role, Role::Drop);
        assert_eq!(v[&2].note.as_deref(), Some("not English (tag fre)"));
        let today = spec87(&tracks, &Rule::default());
        assert_eq!((today[&0].as_str(), today[&1].as_str()), ("keep (review)", "keep (review)"));
        let tracks = measured(vec![eng(0, sdh), eng(1, comm)]);
        let v = checked_decide(&tracks, "en");
        for n in [0, 1] {
            assert_eq!(v[&n], dec(Role::Keep, Some(AMBIGUOUS), Some("no dialogue group")));
        }
    }

    #[test]
    fn mistagged_language_is_kept_for_review() {
        let tracks = measured(vec![track(0, spanish_events(80, 10.0, 3.0), "eng", false)]);
        let v = checked_decide(&tracks, "es");
        assert_eq!((v[&0].role, v[&0].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
    }

    #[test]
    fn unclear_language_is_reviewed_and_never_dropped() {
        let b = base();
        let tracks = measured(vec![eng(0, sdh_version(&b)), eng(1, b.clone()), eng(2, unclear(100, 6, 11.7, 4.0))]);
        assert_eq!(measures_of(&tracks[2]).lang, "unknown");
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&2].role, v[&2].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
        assert!(v[&2].note.as_deref().unwrap_or("").starts_with("language of the text unclear"));
        let tracks = measured(vec![eng(0, dialogue(1700, 1, Words::Nouns, 10.0, 4.0)), eng(1, unclear(160, 6, 11.7, 9.0))]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(
            v[&1],
            dec(Role::Keep, Some(AMBIGUOUS), Some("language of the text unclear (8.7 would drop it as partial)"))
        );
        let tracks = measured(vec![eng(0, unclear(100, 6, 10.0, 4.0))]);
        assert_eq!(
            checked_decide(&tracks, "en")[&0],
            dec(Role::Regular, Some(AMBIGUOUS), Some("language of the text unclear"))
        );
    }

    #[test]
    fn no_language_track_is_read_not_dropped() {
        let tracks = measured(vec![eng(0, base()), track(1, junk(60, 4, 12.0, 4.0), "und", false)]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&0].role, Role::Regular);
        assert_eq!((v[&1].role, v[&1].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
        assert_eq!(spec87(&tracks, &Rule::default())[&1], "drop");
        let tracks = measured(vec![track(0, base(), "und", false)]);
        assert_eq!(checked_decide(&tracks, "en")[&0], dec(Role::Regular, None, None));
    }

    #[test]
    fn no_language_track_in_another_language_or_too_short() {
        let tracks = measured(vec![eng(0, base()), track(1, spanish_events(80, 10.0, 3.0), "und", false)]);
        assert_eq!(checked_decide(&tracks, "en")[&1], dec(Role::Drop, None, Some("not English (tag und)")));
        let tracks = measured(vec![track(0, dialogue(3, 1, Words::Nouns, 10.0, 4.0), "und", false)]);
        assert_eq!(measures_of(&tracks[0]).lang, "short");
        assert_eq!(
            checked_decide(&tracks, "en")[&0],
            dec(Role::Keep, Some(AMBIGUOUS), Some("no language tag and the text does not show its language"))
        );
    }

    #[test]
    fn partial_track_is_a_forced_candidate_or_kept_when_unreadable() {
        let b = dialogue(1700, 1, Words::Nouns, 10.0, 4.0);
        let tracks = measured(vec![eng(0, b.clone()), eng(1, b[100..260].to_vec()), eng(2, junk(160, 5, 11.0, 9.0))]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&0].role, Role::Regular);
        assert_eq!((v[&1].role, v[&1].review.as_deref()), (Role::Forced, None));
        assert_eq!((v[&2].role, v[&2].review.as_deref()), (Role::Keep, Some(AMBIGUOUS)));
        let today = spec87(&tracks, &Rule::default());
        assert_eq!((today[&1].as_str(), today[&2].as_str()), ("drop", "drop"));
    }

    #[test]
    fn readable_partial_track_is_dropped() {
        let other = dialogue(160, 8, Words::Other, 11.7, 9.0);
        let tracks = measured(vec![eng(0, dialogue(1700, 1, Words::Nouns, 10.0, 4.0)), eng(1, other)]);
        assert_eq!(checked_decide(&tracks, "en")[&1], dec(Role::Drop, None, Some("partial_subtitle_dropped")));
    }

    #[test]
    fn unreadable_tracks_are_not_judged_by_their_words() {
        let b = base();
        // An unreadable full track whose words are a subset: 8.7 decides it (two aligned tracks), not T2.
        let tracks = measured(vec![eng(0, b.clone()), eng(1, garbled(&b[40..70]))]);
        assert!(!measures_of(&tracks[1]).readable);
        let v = checked_decide(&tracks, "en");
        assert_eq!((v[&1].role, v[&1].review.as_deref()), (Role::Regular, Some(AMBIGUOUS)));
        assert_eq!(spec87(&tracks, &Rule::default())[&1], "regular");
        // A readable candidate beside an unreadable dialogue track: forced as 8.7 says, but reviewed.
        let tracks = measured(vec![eng(0, garbled(&b)), eng(1, b[40..50].to_vec())]);
        let v = checked_decide(&tracks, "en");
        assert_eq!(v[&1], dec(Role::Forced, Some(AMBIGUOUS), Some("dialogue track unreadable")));
    }

    #[test]
    fn before_yes() {
        // The test's own name hides the function inside this module; the full path reaches it.
        use crate::subtitle::text::before_yes as before_yes_of;
        assert_eq!(
            before_yes_of(&dec(Role::Sdh, None, None), "regular"),
            "regular (review: subtitle_text_disagrees)"
        );
        assert_eq!(before_yes_of(&dec(Role::Regular, None, None), "regular"), "regular");
        assert_eq!(
            before_yes_of(&dec(Role::Regular, Some(AMBIGUOUS), None), "regular"),
            "regular (review: subtitle_text_disagrees)"
        );
        assert_eq!(before_yes_of(&dec(Role::Keep, Some(AMBIGUOUS), None), "keep (review)"), "keep (review)");
        assert_eq!(before_yes_of(&dec(Role::Sdh, None, None), "sdh (review)"), "sdh (review)");
    }
}

// ------------------------------------------------------------------ new: spec87's set order

mod added_rule {
    use super::*;

    #[test]
    fn spec87_breaks_an_event_count_tie_in_cpythons_set_order() {
        // Two English full tracks with the same number of events in one dialogue group: spec87 sorts the group's
        // members by event count with a stable sort over the set, so the set's order picks the SDH track.
        // ((first stream, second stream), the one spec87 called sdh) as Python 3.9.6 printed them.
        let cases: [((i64, i64), i64); 8] = [
            ((3, 11), 11),
            ((11, 3), 3),
            ((2, 10), 2),
            ((10, 2), 10),
            ((0, 8), 0),
            ((8, 0), 8),
            ((0, 1), 0),
            ((1, 0), 0),
        ];
        let b = base();
        let rule = Rule::default();
        for ((first, second), sdh) in cases {
            let tracks = measured(vec![eng(first, b.clone()), eng(second, later(&b, 0.1))]);
            let today = spec87(&tracks, &rule);
            let regular = if sdh == first { second } else { first };
            assert_eq!(
                (today[&sdh].as_str(), today[&regular].as_str()),
                ("sdh", "regular"),
                "s{first} then s{second}"
            );
        }
        // Three tracks joined through each other (the second union is of one set with itself): kept for review.
        let tracks = measured(vec![eng(16, b.clone()), eng(0, later(&b, 0.1)), eng(8, later(&b, 0.2))]);
        let today = spec87(&tracks, &rule);
        for n in [16, 0, 8] {
            assert_eq!(today[&n], "keep (review)");
        }
    }
}
// ------------------------------------------------------------------ the files of a title

use crate::recording::{py_splitlines, MemTree};
use serde_json::json;

/// The packets of displayed events as Files.write_title writes them: a 4000-byte packet at each start and a 30-byte
/// clear packet at its end, times as '%.3f'.
fn packets_of(events: &[Event]) -> serde_json::Value {
    let mut out = Vec::new();
    for e in events {
        out.push(json!({"pts_time": py_fixed(e.start, 3), "duration_time": py_fixed(e.dur, 3), "size": "4000"}));
        out.push(json!({"pts_time": py_fixed(e.start + e.dur, 3), "duration_time": "0.021", "size": "30"}));
    }
    serde_json::Value::from(out)
}

/// (sub-<n>.frames.json, sub-<n>.ocr.jsonl) as Files.write_title writes them: per event a frame without text and one
/// with the event's lines, as a "lines" list or as the proof of concept's ' | '-joined "text".
fn ocr_files(events: &[Event], lines_format: bool) -> (String, String) {
    let (mut frames, mut recs) = (Vec::new(), Vec::new());
    for (k, e) in events.iter().enumerate() {
        let t = py_round(e.start - events[0].start, 2);
        frames.push(json!({"frame": 2 * k, "lines": 0}));
        frames.push(json!({"frame": 2 * k + 1, "lines": e.lines.len()}));
        recs.push(json!({"frame": 2 * k, "file": "f.png", "t": py_max0(t - 0.5), "text": ""}).to_string());
        let with_text = if lines_format {
            json!({"frame": 2 * k + 1, "file": "f.png", "t": t, "lines": e.lines})
        } else {
            json!({"frame": 2 * k + 1, "file": "f.png", "t": t, "text": e.lines.join(" | ")})
        };
        recs.push(with_text.to_string());
    }
    (serde_json::Value::from(frames).to_string(), recs.join("\n"))
}

/// The files of Files.write_title, by path below the temporary folder: rec/subs/index.json with one English PGS
/// stream, its packets, rec/subs-ocr/sub-0.frames.json and text/sub-0.ocr.jsonl; `n` is the number of events.
struct Title {
    files: BTreeMap<String, String>,
    n: usize,
}

/// Py: Files.write_title(root, lines_format).
fn write_title(lines_format: bool) -> Title {
    let ev = sdh_version(&dialogue(60, 1, Words::Nouns, 10.0, 4.0));
    let (frames, jsonl) = ocr_files(&ev, lines_format);
    let mut files = BTreeMap::new();
    files.insert(
        "rec/subs/index.json".to_string(),
        json!([{"n": 0, "language": "eng", "codec": "HDMV PGS", "forced": false}]).to_string(),
    );
    files.insert("rec/subs/sub-0.packets.json".to_string(), json!({"packets": packets_of(&ev)}).to_string());
    files.insert("rec/subs-ocr/sub-0.frames.json".to_string(), frames);
    files.insert("text/sub-0.ocr.jsonl".to_string(), jsonl);
    Title { files, n: ev.len() }
}

impl Title {
    /// The files as a tree (the folders exist even when a test removed their files).
    fn tree(&self) -> MemTree {
        let mut tree = MemTree::new("tmp", "tmp");
        for dir in ["rec/subs", "rec/subs-ocr", "text"] {
            tree.insert_dir(dir);
        }
        for (path, text) in &self.files {
            tree.insert_file(path, text.clone().into_bytes());
        }
        tree
    }

    /// Py: Files.add_stream(rec, n, language, codec, packets).
    fn add_stream(&mut self, n: i64, language: &str, codec: &str, packets: serde_json::Value) {
        let mut index: Vec<serde_json::Value> = serde_json::from_str(&self.files["rec/subs/index.json"]).unwrap();
        index.push(json!({"n": n, "language": language, "codec": codec, "forced": false}));
        self.files.insert("rec/subs/index.json".to_string(), serde_json::Value::from(index).to_string());
        self.files.insert(format!("rec/subs/sub-{n}.packets.json"), json!({"packets": packets}).to_string());
    }
}

/// st.main(['--rec', rec, '--text', text] + orig) over the title's files: (exit code, stdout, stderr).
fn main_run(title: &Title, orig_lang: Option<&str>) -> (i32, String, String) {
    let tree = title.tree();
    let run = subs_run(&tree, &PathKey::new("rec"), &tree, &PathKey::new("text"), orig_lang, 64, &Rule::default());
    let text = |lines: &[String]| lines.iter().map(|line| format!("{line}\n")).collect::<String>();
    (run.exit_code, text(&run.stdout), text(&run.stderr))
}

// ------------------------------------------------------------------ Files

mod files {
    use super::*;

    #[test]
    fn main_prints_numbers_and_a_verdict() {
        for lines_format in [false, true] {
            let title = write_title(lines_format);
            let (code, out, _) = main_run(&title, Some("en"));
            let n = title.n;
            assert_eq!(code, 0);
            assert!(out.contains(&format!("stream s0 eng: events={n}")), "{out}");
            assert!(out.contains("class=sdh"), "{out}");
            assert!(out.contains("text by order"), "{out}");
            assert!(out.contains(&format!("| s0 | eng | {n} | sdh | sdh |")), "{out}");
            for word in NOUNS.iter().chain(VERBS.iter()) {
                assert!(!out.contains(word.as_str()), "the made-up word {word} was printed");
            }
        }
    }

    #[test]
    fn pairing_by_time_when_counts_differ() {
        let mut title = write_title(true);
        let rows: Vec<String> =
            py_splitlines(&title.files["text/sub-0.ocr.jsonl"]).iter().map(|row| row.to_string()).collect();
        title.files.insert("text/sub-0.ocr.jsonl".to_string(), rows[..rows.len() - 4].join("\n"));
        let tree = title.tree();
        let tracks = load_title(&tree, &PathKey::new("rec"), &tree, &PathKey::new("text"), 64).unwrap();
        assert!(tracks[0].pairing.as_deref().unwrap_or("").contains("text by time"), "{:?}", tracks[0].pairing);
        assert_eq!(tracks[0].events.iter().filter(|e| !e.lines.is_empty()).count(), title.n - 2);
    }

    #[test]
    fn closed_captions_are_set_aside_and_unread_streams_reported() {
        let mut title = write_title(true);
        let n = title.n;
        // The DVD closed captions as MakeMKV writes them: SubRip, every packet an event (spec 8.7), no OCR text.
        let cc: Vec<serde_json::Value> = (0..5)
            .map(|i| json!({"pts_time": py_fixed(20.0 + 3.0 * i as f64, 3), "duration_time": "2.000", "size": "40"}))
            .collect();
        title.add_stream(1, "eng", "SubRip/SRT", serde_json::Value::from(cc));
        let pgs: serde_json::Value =
            serde_json::from_str::<serde_json::Value>(&title.files["rec/subs/sub-0.packets.json"]).unwrap()["packets"]
                .clone();
        title.add_stream(2, "spa", "HDMV PGS", pgs.clone());
        let (code, out, _) = main_run(&title, Some("en"));
        assert_eq!(code, 0);
        assert!(out.contains("1 subtitle streams with OCR text, 1 not read (another language), 1 set aside"), "{out}");
        assert!(
            out.contains(
                "stream s1 eng: set aside (closed captions: SubRip/SRT is a text stream; 8.7 sets it aside), 5 events"
            ),
            "{out}"
        );
        assert!(out.contains("| s1 | eng | 5 | set aside | set aside | - |"), "{out}");
        assert!(out.contains("| set aside | set aside |\n"), "{out}");
        assert!(out.contains(&format!("stream s2 spa: not read (no OCR text), {n} events, used for timing only")), "{out}");
        assert!(
            out.contains(&format!("| s2 | spa | {n} | not read | drop | - | not English (tag spa) | drop | drop |")),
            "{out}"
        );
        assert!(
            out.contains(&format!(
                "| s0 | eng | {n} | sdh | sdh | - | - | regular | regular (review: subtitle_text_disagrees) |"
            )),
            "{out}"
        );
        // An English bitmap stream without OCR text is an input error, never left out silently.
        title.add_stream(3, "eng", "HDMV PGS", pgs);
        let (code, _, err) = main_run(&title, None);
        assert_eq!(code, 2);
        assert!(err.contains("stream s3 eng: no OCR text"), "{err}");
    }

    #[test]
    fn missing_text_is_an_input_error() {
        let mut title = write_title(true);
        title.files.remove("text/sub-0.ocr.jsonl");
        let (code, _, _) = main_run(&title, None);
        assert_eq!(code, 2);
    }
}

// ------------------------------------------------------------------ new: a whole title against the Python

mod added_title {
    use super::*;

    /// A title of seven streams built with the generators above and written as Files.write_title writes its one:
    /// s0 English SDH (with a note-only event), s1 SubRip closed captions without text, s2 Spanish without text,
    /// s3 English regular in the proof of concept's ' | ' text with a second line "| went home." in every sixth
    /// event, s4 English flagged forced, s5 a no-language stream of OCR junk, s6 English commentary. With
    /// `tmdb`, the recording's tmdb.json names that original language.
    fn mixed_title(tmdb: Option<&str>) -> MemTree {
        let mut ev0 = sdh_version(&dialogue(60, 1, Words::Nouns, 10.0, 4.0));
        let gap = ev0[10].start + 3.3;
        ev0.push(Event { start: gap, dur: 0.5, lines: vec!["#".to_string()] });
        sort_by_start(&mut ev0);
        let mut ev3 = later(&dialogue(60, 1, Words::Nouns, 10.0, 4.0), 0.1);
        for k in (0..60).step_by(6) {
            ev3[k].lines.push("| went home.".to_string());
        }
        let ev4 = dialogue(60, 1, Words::Nouns, 10.0, 4.0)[20..30].to_vec();
        let ev5 = junk(30, 4, 12.0, 4.0);
        let ev6 = dialogue(60, 5, Words::Other, 11.7, 4.0);
        let cc: Vec<serde_json::Value> = (0..5)
            .map(|i| json!({"pts_time": py_fixed(20.0 + 3.0 * i as f64, 3), "duration_time": "2.000", "size": "40"}))
            .collect();
        // (n, language, codec, forced, packets, the OCR text's events and whether they are written as "lines")
        let streams: Vec<(i64, &str, &str, bool, serde_json::Value, Option<(&[Event], bool)>)> = vec![
            (0, "eng", "HDMV PGS", false, packets_of(&ev0), Some((ev0.as_slice(), true))),
            (1, "eng", "SubRip/SRT", false, serde_json::Value::from(cc), None),
            (2, "spa", "HDMV PGS", false, packets_of(&ev0), None),
            (3, "eng", "HDMV PGS", false, packets_of(&ev3), Some((ev3.as_slice(), false))),
            (4, "eng", "HDMV PGS", true, packets_of(&ev4), Some((ev4.as_slice(), true))),
            (5, "und", "HDMV PGS", false, packets_of(&ev5), Some((ev5.as_slice(), true))),
            (6, "eng", "HDMV PGS", false, packets_of(&ev6), Some((ev6.as_slice(), true))),
        ];
        let mut tree = MemTree::new(".", "e2e");
        let mut index = Vec::new();
        for (n, language, codec, forced, packets, read) in &streams {
            index.push(json!({"n": n, "language": language, "codec": codec, "forced": forced}));
            tree.insert_file(&format!("rec/subs/sub-{n}.packets.json"), json!({"packets": packets}).to_string().into_bytes());
            if let Some((events, lines_format)) = read {
                let (frames, jsonl) = ocr_files(events, *lines_format);
                tree.insert_file(&format!("rec/subs-ocr/sub-{n}.frames.json"), frames.into_bytes());
                tree.insert_file(&format!("text/sub-{n}.ocr.jsonl"), jsonl.into_bytes());
            }
        }
        tree.insert_file("rec/subs/index.json", serde_json::Value::from(index).to_string().into_bytes());
        if let Some(language) = tmdb {
            let info = json!({"original_language": language, "fetched_at": "2026-10-13"});
            tree.insert_file("tmdb.json", info.to_string().into_bytes());
        }
        tree
    }

    /// What the Python printed for mixed_title(None) with --orig-lang en (Python 3.9.6, 2026-10-13).
    const E2E_EN: &[&str] = &[
        "rec: 5 subtitle streams with OCR text, 1 not read (another language), 1 set aside (closed captions); rule thresholds are provisional",
        "stream s0 eng: events=69 lines=69 words=600 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.600 es=0.000 fr=0.000 de=0.000) CUE=8 LABEL=6 CHEV=0 M=14 rate=0.2029 SO=8 CS=8 caps_share=0.000 CAPS_CUE=0 NOTE=1 NOTE_LOOSE=0 ODD=0 class=sdh  [text by order, 69 frames with text, 0 unmatched]",
        "  note-only events: 1 (left out of letters)",
        "stream s1 eng: set aside (closed captions: SubRip/SRT is a text stream; 8.7 sets it aside), 5 events",
        "stream s2 spa: not read (no OCR text), 69 events, used for timing only",
        "stream s3 eng: events=60 lines=70 words=630 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.587 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=regular  [text by order, 60 frames with text, 0 unmatched]",
        "  corrections: pipe_I=10",
        "stream s4 eng: events=10 lines=10 words=100 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.600 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=regular  [text by order, 10 frames with text, 0 unmatched]",
        "stream s5 und: events=30 lines=30 words=64 letters=0.867 wordlike=0.033 readable=no lang=unknown (en=0.000 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=5 NOTE_LOOSE=2 ODD=0 class=unreadable  [text by order, 30 frames with text, 0 unmatched]",
        "stream s6 eng: events=60 lines=60 words=600 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.600 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=regular  [text by order, 60 frames with text, 0 unmatched]",
        "pair s0 s3: aligned=0.870/1.000 timing=same CB=1.000/0.971 content=same UC=0/0 subset=no",
        "pair s0 s4: aligned=0.145/1.000 timing=same CB=0.183/1.000 content=partial UC=6/0 subset=s4 in s0",
        "pair s0 s5: aligned=0.000/0.000 timing=diff CB=0.000/n/a content=unknown UC=3/0 subset=no",
        "pair s0 s6: aligned=0.000/0.000 timing=diff CB=0.000/0.000 content=different UC=0/0 subset=no",
        "pair s3 s4: aligned=0.167/1.000 timing=same CB=0.178/1.000 content=partial UC=0/0 subset=s4 in s3",
        "pair s3 s5: aligned=0.000/0.000 timing=diff CB=0.000/n/a content=unknown UC=0/0 subset=no",
        "pair s3 s6: aligned=0.000/0.000 timing=diff CB=0.000/0.000 content=different UC=0/0 subset=no",
        "pair s4 s5: aligned=0.000/0.000 timing=diff CB=0.000/n/a content=unknown UC=0/0 subset=no",
        "pair s4 s6: aligned=0.000/0.000 timing=diff CB=0.000/0.000 content=different UC=0/0 subset=no",
        "pair s5 s6: aligned=1.000/0.500 timing=same CB=n/a/0.000 content=unknown UC=0/0 subset=no",
        "original language: en",
        "| Stream | Tag | Events | Class | Rule | Review | Note | 8.7 today | Before yes |",
        "|---|---|---|---|---|---|---|---|---|",
        "| s0 | eng | 69 | sdh | sdh | - | - | sdh | sdh |",
        "| s1 | eng | 5 | set aside | set aside | - | closed captions, a text stream (8.7: set aside first) | set aside | set aside |",
        "| s2 | spa | 69 | not read | drop | - | not English (tag spa) | drop | drop |",
        "| s3 | eng | 60 | regular | regular | - | - | regular | regular |",
        "| s4 | eng | 10 | regular | forced | - | words found in the dialogue track | forced | forced |",
        "| s5 | und | 30 | unreadable | keep | subtitle_ambiguous | no language tag and the text does not show its language | drop | drop (review: subtitle_text_disagrees) |",
        "| s6 | eng | 60 | regular | drop | - | commentary or trivia: other words and other timing | drop | drop |",
        "row: | rec/evidence.mks | s0: HDMV PGS eng, 69 events | M 14, SO 8, CS 8, lang en, sdh: sdh | |",
        "row: | rec/evidence.mks | s1: SubRip/SRT eng, 5 events | set aside (closed captions): set aside | |",
        "row: | rec/evidence.mks | s2: HDMV PGS spa, 69 events | not read (timing only): drop | |",
        "row: | rec/evidence.mks | s3: HDMV PGS eng, 60 events | M 0, SO 0, CS 0, lang en, regular: regular | |",
        "row: | rec/evidence.mks | s4: HDMV PGS eng, 10 events | M 0, SO 0, CS 0, lang en, regular: forced | |",
        "row: | rec/evidence.mks | s5: HDMV PGS und, 30 events | M 0, SO 0, CS 0, lang unknown, unreadable: keep (review: no language tag and the text does not show its language) | |",
        "row: | rec/evidence.mks | s6: HDMV PGS eng, 60 events | M 0, SO 0, CS 0, lang en, regular: drop | |",
    ];

    /// What the Python printed for mixed_title(Some("fr")) without --orig-lang: the original language from tmdb.json.
    const E2E_FR: &[&str] = &[
        "rec: 5 subtitle streams with OCR text, 1 not read (another language), 1 set aside (closed captions); rule thresholds are provisional",
        "stream s0 eng: events=69 lines=69 words=600 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.600 es=0.000 fr=0.000 de=0.000) CUE=8 LABEL=6 CHEV=0 M=14 rate=0.2029 SO=8 CS=8 caps_share=0.000 CAPS_CUE=0 NOTE=1 NOTE_LOOSE=0 ODD=0 class=sdh  [text by order, 69 frames with text, 0 unmatched]",
        "  note-only events: 1 (left out of letters)",
        "stream s1 eng: set aside (closed captions: SubRip/SRT is a text stream; 8.7 sets it aside), 5 events",
        "stream s2 spa: not read (no OCR text), 69 events, used for timing only",
        "stream s3 eng: events=60 lines=70 words=630 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.587 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=regular  [text by order, 60 frames with text, 0 unmatched]",
        "  corrections: pipe_I=10",
        "stream s4 eng: events=10 lines=10 words=100 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.600 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=regular  [text by order, 10 frames with text, 0 unmatched]",
        "stream s5 und: events=30 lines=30 words=64 letters=0.867 wordlike=0.033 readable=no lang=unknown (en=0.000 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=5 NOTE_LOOSE=2 ODD=0 class=unreadable  [text by order, 30 frames with text, 0 unmatched]",
        "stream s6 eng: events=60 lines=60 words=600 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.600 es=0.000 fr=0.000 de=0.000) CUE=0 LABEL=0 CHEV=0 M=0 rate=0.0000 SO=0 CS=0 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=regular  [text by order, 60 frames with text, 0 unmatched]",
        "pair s0 s3: aligned=0.870/1.000 timing=same CB=1.000/0.971 content=same UC=0/0 subset=no",
        "pair s0 s4: aligned=0.145/1.000 timing=same CB=0.183/1.000 content=partial UC=6/0 subset=s4 in s0",
        "pair s0 s5: aligned=0.000/0.000 timing=diff CB=0.000/n/a content=unknown UC=3/0 subset=no",
        "pair s0 s6: aligned=0.000/0.000 timing=diff CB=0.000/0.000 content=different UC=0/0 subset=no",
        "pair s3 s4: aligned=0.167/1.000 timing=same CB=0.178/1.000 content=partial UC=0/0 subset=s4 in s3",
        "pair s3 s5: aligned=0.000/0.000 timing=diff CB=0.000/n/a content=unknown UC=0/0 subset=no",
        "pair s3 s6: aligned=0.000/0.000 timing=diff CB=0.000/0.000 content=different UC=0/0 subset=no",
        "pair s4 s5: aligned=0.000/0.000 timing=diff CB=0.000/n/a content=unknown UC=0/0 subset=no",
        "pair s4 s6: aligned=0.000/0.000 timing=diff CB=0.000/0.000 content=different UC=0/0 subset=no",
        "pair s5 s6: aligned=1.000/0.500 timing=same CB=n/a/0.000 content=unknown UC=0/0 subset=no",
        "original language: fr",
        "| Stream | Tag | Events | Class | Rule | Review | Note | 8.7 today | Before yes |",
        "|---|---|---|---|---|---|---|---|---|",
        "| s0 | eng | 69 | sdh | sdh | - | - | sdh | sdh |",
        "| s1 | eng | 5 | set aside | set aside | - | closed captions, a text stream (8.7: set aside first) | set aside | set aside |",
        "| s2 | spa | 69 | not read | drop | - | not English (tag spa) | drop | drop |",
        "| s3 | eng | 60 | regular | regular | - | - | regular | regular |",
        "| s4 | eng | 10 | regular | forced | - | words found in the dialogue track | forced | forced |",
        "| s5 | und | 30 | unreadable | keep | subtitle_ambiguous | no language tag and the text does not show its language | drop | drop (review: subtitle_text_disagrees) |",
        "| s6 | eng | 60 | regular | keep | subtitle_ambiguous | original language fr | drop | drop (review: subtitle_text_disagrees) |",
        "row: | rec/evidence.mks | s0: HDMV PGS eng, 69 events | M 14, SO 8, CS 8, lang en, sdh: sdh | |",
        "row: | rec/evidence.mks | s1: SubRip/SRT eng, 5 events | set aside (closed captions): set aside | |",
        "row: | rec/evidence.mks | s2: HDMV PGS spa, 69 events | not read (timing only): drop | |",
        "row: | rec/evidence.mks | s3: HDMV PGS eng, 60 events | M 0, SO 0, CS 0, lang en, regular: regular | |",
        "row: | rec/evidence.mks | s4: HDMV PGS eng, 10 events | M 0, SO 0, CS 0, lang en, regular: forced | |",
        "row: | rec/evidence.mks | s5: HDMV PGS und, 30 events | M 0, SO 0, CS 0, lang unknown, unreadable: keep (review: no language tag and the text does not show its language) | |",
        "row: | rec/evidence.mks | s6: HDMV PGS eng, 60 events | M 0, SO 0, CS 0, lang en, regular: keep (review: original language fr) | |",
    ];

    /// The keys of measure's dict, in its order (Python 3.9.6: list(t['m'].keys())).
    const MEASURE_KEYS: [&str; 26] = [
        "events", "text_events", "lines", "words", "note_only", "letter_events", "wordlike", "readable", "lang",
        "shares", "caps_share", "caps_style", "CUE", "LABEL", "CHEV", "M", "SO", "CS", "rate", "CAPS_CUE", "NOTE",
        "NOTE_LOOSE", "ODD", "corrections", "class", "why",
    ];

    #[test]
    fn subs_run_prints_the_pythons_lines() {
        let tree = mixed_title(None);
        let run = subs_run(&tree, &PathKey::new("rec"), &tree, &PathKey::new("text"), Some("en"), 64, &Rule::default());
        assert_eq!((run.exit_code, run.stderr.len()), (0, 0), "{:?}", run.stderr);
        assert_eq!(run.stdout, E2E_EN);
        assert_eq!(run.orig_lang.as_deref(), Some("en"));
        // No --orig-lang: the original language comes from <rec>/../tmdb.json, and T6 keeps the commentary.
        let tree = mixed_title(Some("fr"));
        let run = subs_run(&tree, &PathKey::new("rec"), &tree, &PathKey::new("text"), None, 64, &Rule::default());
        assert_eq!((run.exit_code, run.stderr.len()), (0, 0), "{:?}", run.stderr);
        assert_eq!(run.stdout, E2E_FR);
        assert_eq!(run.orig_lang.as_deref(), Some("fr"));
    }

    #[test]
    fn subs_json_holds_numbers_and_decisions_only() {
        let tree = mixed_title(None);
        let text = PathKey::new("text");
        let run = subs_run(&tree, &PathKey::new("rec"), &tree, &text, Some("en"), 64, &Rule::default());
        let doc = subs_json(&run, "text", &tree, &text);
        let written = serde_json::to_string(&doc).unwrap();
        // No line, word or word pair of the OCR text reaches the document.
        for word in NOUNS.iter().chain(VERBS.iter()).chain(OTHER.iter()) {
            assert!(!written.contains(word.as_str()), "the made-up word {word} was written");
        }
        assert_eq!((doc.reading.as_str(), doc.exit_code, doc.orig_lang.as_deref()), ("text", 0, Some("en")));
        let statuses: Vec<&str> = doc.streams.iter().map(|s| s.status.as_str()).collect();
        assert_eq!(statuses, ["read", "set aside", "not read", "read", "read", "read", "read"]);
        // The outcomes are the table's: decide's role, 8.7 today, before yes.
        let outcomes: Vec<String> = doc
            .streams
            .iter()
            .map(|s| {
                let role = s.decide.as_ref().expect("a decision").role.as_str();
                format!("s{} {} {} {}", s.n, role, s.spec87.as_deref().unwrap(), s.before_yes.as_deref().unwrap())
            })
            .collect();
        assert_eq!(
            outcomes,
            [
                "s0 sdh sdh sdh",
                "s1 set aside set aside set aside",
                "s2 drop drop drop",
                "s3 regular regular regular",
                "s4 forced forced forced",
                "s5 keep drop drop (review: subtitle_text_disagrees)",
                "s6 drop drop drop",
            ]
        );
        assert_eq!(doc.pairs.len(), 10);
        assert_eq!((doc.pairs[1].a, doc.pairs[1].b, doc.pairs[1].subset.as_str()), (0, 4, "s4 in s0"));
        let hashed: Vec<&str> = doc.text_sha1.iter().map(|(name, _)| name).collect();
        assert_eq!(hashed, ["sub-0.ocr.jsonl", "sub-3.ocr.jsonl", "sub-4.ocr.jsonl", "sub-5.ocr.jsonl", "sub-6.ocr.jsonl"]);
        // The measures keep the Python dict's key order, and the document reads back as it was written.
        let value = serde_json::to_value(&doc).unwrap();
        let keys: Vec<&str> =
            value["streams"][0]["m"].as_object().expect("measures").keys().map(String::as_str).collect();
        assert_eq!(keys, MEASURE_KEYS);
        assert!(value["streams"][1]["m"].is_null());
        let pair_keys: Vec<&str> = value["pairs"][0].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(pair_keys, ["a", "b", "aligned", "cb", "uc", "timing", "content", "subset"]);
        let back: SubsJson = serde_json::from_value(value).unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn subs_run_reports_unreadable_input_as_the_python() {
        let title = |packets: &str| {
            let mut tree = MemTree::new("tmp", "tmp");
            tree.insert_file("rec/subs/index.json", br#"[{"n": 0, "language": "eng", "codec": "HDMV PGS"}]"#.to_vec());
            tree.insert_file("rec/subs/sub-0.packets.json", packets.as_bytes().to_vec());
            tree.insert_file("text/sub-0.ocr.jsonl", b"{\"lines\": [\"x\"]}\n".to_vec());
            tree
        };
        let rec = PathKey::new("rec");
        let text = PathKey::new("text");
        let rule = Rule::default();
        let tree = title(r#"{"packets": [{"pts_time": "x", "size": "4000"}]}"#);
        let run = subs_run(&tree, &rec, &tree, &text, None, 64, &rule);
        assert_eq!((run.exit_code, run.stdout.len()), (2, 0));
        assert_eq!(run.stderr, ["cannot read the input: could not convert string to float: 'x'"]);
        let tree = title(r#"{"packets": [{"pts_time": "1.000", "size": "4k"}]}"#);
        let run = subs_run(&tree, &rec, &tree, &text, None, 64, &rule);
        assert_eq!(run.stderr, ["cannot read the input: invalid literal for int() with base 10: '4k'"]);
        let empty = MemTree::new("tmp", "tmp");
        let run = subs_run(&empty, &rec, &empty, &text, None, 64, &rule);
        assert_eq!(run.stderr, ["cannot read the input: [Errno 2] No such file or directory: 'tmp/rec/subs/index.json'"]);
        // A tmdb.json that is not JSON crashed the Python; the port says so and exits 2 (port spec 3.4).
        let mut tree = title(r#"{"packets": [{"pts_time": "1.000", "size": "4000"}]}"#);
        tree.insert_file("tmdb.json", b"not json".to_vec());
        let run = subs_run(&tree, &rec, &tree, &text, None, 64, &rule);
        assert_eq!((run.exit_code, run.stdout.len()), (2, 0));
        assert!(run.stderr[0].starts_with("cannot read the input: "), "{:?}", run.stderr);
        assert_eq!(run.analysis, None);
    }
}
