//! Tests of core::audio::rule. Ported one for one from scripts/phase0/test_audio_similarity.py, classes Rule (5),
//! ReverseRun (5), CrossLanguage (6) and PositionVeto (4), and from scripts/phase0/test_audio_dataset.py, class
//! Similarity (1): one module per class, each test under its Python name without `test_` (as Task 4 names the
//! Measures tests). The modules rule_set, spellings and results_contract are new. The synthetic level series are the
//! Python's (crate::testsupport over PyRandom).

use std::sync::OnceLock;

use crate::audio::measures::Constants;
use crate::audio::rule::{results, FolderResults, Plain, Rule};
use crate::recording::{MemTree, PathKey};
use crate::testsupport::{commentary, programme};

/// MIX, CENTRE = programme(): 420 s of a dialogue-driven programme (seed 1), computed once.
fn programme_pair() -> &'static (Vec<f64>, Vec<f64>) {
    static PAIR: OnceLock<(Vec<f64>, Vec<f64>)> = OnceLock::new();
    PAIR.get_or_init(|| programme(420, 1))
}

/// MIX.
fn mix() -> &'static [f64] {
    &programme_pair().0
}

/// commentary(MIX) (seed 7), computed once; the Python builds the same list at every call.
fn comment() -> &'static [f64] {
    static COMMENT: OnceLock<Vec<f64>> = OnceLock::new();
    COMMENT.get_or_init(|| commentary(mix(), 7))
}

/// Py: test_audio_similarity.py, class Rule (5 tests).
mod rule {
    use crate::audio::measures::{Measures, Reverse, Run};
    use crate::audio::rule::{verdict, Rule, Verdict};

    /// Rule.BASE: the measured commentary (the keys the Python dict has; Run::blank() gives the others).
    fn base() -> Run {
        Run {
            overlap: 1.0,
            main_silent: 0.03,
            gap_s: 355.0,
            dc: 0.03,
            ag: 0.66,
            ag_s: 234.0,
            adl: 0.18,
            tenths: 10,
            tenths_comm: 10,
            tenths_ad: 10,
            smi: 0.58,
            ls_db: 10.1,
            added_s: 518.0,
            rev: Reverse::Earlier { rev_gap_s: 29.0, rev_ag: 0.10 },
            ..Run::blank()
        }
    }

    /// Rule.v: sim.verdict(dict(BASE, **change)); the caller writes the change with struct update syntax.
    fn v(m: Run) -> Verdict {
        verdict(&Measures::Run(m), &Rule::default())
    }

    #[test]
    fn the_measured_commentary_has_the_pattern() {
        assert_eq!(v(base()), Verdict::Commentary);
    }

    #[test]
    fn each_condition_is_required() {
        assert_eq!(v(Run { dc: 0.31, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { ag: 0.49, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { tenths_comm: 8, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { tenths: 5, tenths_comm: 5, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { smi: 0.49, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { ls_db: 13.1, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { added_s: 59.0, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { main_silent: 0.11, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { gap_s: 89.0, ..base() }), Verdict::NoteCommentary);
        assert_eq!(v(Run { overlap: 0.89, ..base() }), Verdict::NoteCommentary);
    }

    #[test]
    fn a_main_track_that_adds_sound_to_the_candidate_makes_the_reference_unclear() {
        let earlier = |rev_gap_s, rev_ag| Run { rev: Reverse::Earlier { rev_gap_s, rev_ag }, ..base() };
        assert_eq!(v(earlier(150.0, 0.35)), Verdict::ReferenceUnclear);
        assert_eq!(v(earlier(60.0, 0.35)), Verdict::Commentary);
    }

    #[test]
    fn audio_description_is_only_ever_a_note() {
        let ad = Run {
            dc: 0.95,
            adl: 0.0,
            ag: 0.30,
            ag_s: 100.0,
            tenths_comm: 2,
            tenths_ad: 9,
            smi: 0.55,
            ls_db: 17.0,
            ..base()
        };
        assert_eq!(v(ad.clone()), Verdict::AdPattern);
        assert_eq!(v(Run { smi: 0.30, ..ad }), Verdict::NoteAd);
    }

    #[test]
    fn a_copy_is_kept() {
        let copy = Run {
            dc: 1.0,
            ag: 0.0,
            ag_s: 0.0,
            adl: 0.0,
            tenths_comm: 0,
            tenths_ad: 0,
            smi: f64::NAN,
            ..base()
        };
        assert_eq!(v(copy), Verdict::Keep);
    }
}

/// Py: test_audio_similarity.py, class ReverseRun (5 tests). EXP-13 addendum 9.3: the reverse run is judged with the
/// commentary pattern itself (no number of its own).
mod reverse_run {
    use super::{comment, mix};
    use crate::audio::measures::{analyse, swapped, Constants, Measures, RevBlock, Reverse, Run};
    use crate::audio::rule::{not_judged, reverse_pattern, verdict, Rule, Verdict};
    use crate::testsupport::{power_sum, speech, PyRandom, FPS};

    /// ReverseRun.PANS: the English commentary of Pan's Labyrinth against the Spanish main track, as measured
    /// (all-tracks, 1-2-3), with its two earlier reverse keys (rev_gap_s 470.0, rev_ag 0.31).
    fn pans() -> Run {
        Run {
            overlap: 1.0,
            main_silent: 0.0,
            gap_s: 929.0,
            dc: 0.08,
            ag: 0.54,
            ag_s: 505.0,
            adl: 0.18,
            tenths: 10,
            tenths_comm: 9,
            tenths_ad: 10,
            smi: 0.51,
            ls_db: 10.8,
            added_s: 2016.0,
            rev: Reverse::Earlier { rev_gap_s: 470.0, rev_ag: 0.31 },
            ..Run::blank()
        }
    }

    /// PANS's rev_gap_s and rev_ag with ReverseRun.REV: the reverse block of dict(PANS, **REV).
    fn rev() -> RevBlock {
        RevBlock {
            rev_error: String::new(),
            rev_overlap: 1.0,
            rev_main_silent: 0.001,
            rev_gap_s: 470.0,
            rev_dc: 0.06,
            rev_ag: 0.31,
            rev_ag_s: 144.0,
            rev_adl: 0.04,
            rev_tenths: Some(10),
            rev_tenths_comm: Some(1),
            rev_tenths_ad: Some(10),
            rev_smi: 0.39,
            rev_smi_windows: Some(526),
            rev_ls_db: 17.1,
            rev_added_s: 796.0,
            rev_gain_db: -6.7,
        }
    }

    /// ReverseRun.m: dict(dict(PANS, **REV), **change); `change` sets forward keys on the Run, rev_ keys on the block.
    fn m(change: impl FnOnce(&mut Run, &mut RevBlock)) -> Measures {
        let (mut run, mut block) = (pans(), rev());
        change(&mut run, &mut block);
        run.rev = Reverse::Block(block);
        Measures::Run(run)
    }

    /// ReverseRun.v.
    fn v(change: impl FnOnce(&mut Run, &mut RevBlock)) -> Verdict {
        verdict(&m(change), &Rule::default())
    }

    /// The `pattern` dict of the Python test_a_reverse_run_with_the_commentary_pattern_makes_the_reference_unclear.
    fn pattern(b: &mut RevBlock) {
        b.rev_dc = 0.03;
        b.rev_ag = 0.66;
        b.rev_tenths_comm = Some(10);
        b.rev_smi = 0.58;
        b.rev_ls_db = 10.1;
        b.rev_added_s = 517.0;
    }

    /// film(seconds, seed): a film-like programme, lines of 2-3 s, then 2.5-4 s of quieter sound (music, effects),
    /// so that its level spread is a film's (about 23 dB). Returns (mix, centre). Py: test_audio_similarity.film, in
    /// its order of random draws.
    fn film(seconds: usize, seed: u64) -> (Vec<f64>, Vec<f64>) {
        let mut rnd = PyRandom::new(seed);
        let (mut mix, mut centre) = (Vec::new(), Vec::new());
        while mix.len() < seconds * FPS {
            let frames = rnd.randint(2 * FPS as i64, 3 * FPS as i64) as usize;
            let line = speech(&mut rnd, frames, -20.0, -30.0);
            mix.extend_from_slice(&line);
            centre.extend(line.iter().map(|v| v - 1.0));
            let n = rnd.randint((2.5 * FPS as f64) as i64, 4 * FPS as i64);
            for _ in 0..n {
                mix.push(-45.0 + rnd.uniform(-3.0, 3.0));
            }
            for _ in 0..n {
                centre.push(-75.0 + rnd.uniform(-1.0, 1.0));
            }
        }
        mix.truncate(seconds * FPS);
        centre.truncate(seconds * FPS);
        (mix, centre)
    }

    #[test]
    fn a_commentary_whose_film_plays_in_its_pauses_keeps_its_verdict() {
        assert_eq!(v(|_, _| {}), Verdict::Commentary);
        // measures written before the reverse block existed are judged by the earlier reverse test, which blocked it
        assert_eq!(verdict(&Measures::Run(pans()), &Rule::default()), Verdict::ReferenceUnclear);
    }

    #[test]
    fn a_reverse_run_with_the_commentary_pattern_makes_the_reference_unclear() {
        assert_eq!(v(|_, b| pattern(b)), Verdict::ReferenceUnclear);
        // the reverse run cannot be judged
        assert_eq!(v(|_, b| { pattern(b); b.rev_gap_s = 29.0; }), Verdict::Commentary);
        // no pattern: a condition fails
        assert_eq!(v(|_, b| { pattern(b); b.rev_ls_db = 13.1; }), Verdict::Commentary);
        assert_eq!(v(|_, b| { pattern(b); b.rev_error = "too short".to_string(); }), Verdict::Commentary);
        let unjudged = |r: &mut Run, b: &mut RevBlock| {
            pattern(b);
            r.gap_s = 29.0;
            r.dc = 0.02;
            r.ag = 0.10;
            r.tenths = 0;
            r.tenths_comm = 0;
        };
        // whatever the forward run says
        assert_eq!(v(unjudged), Verdict::ReferenceUnclear);
        assert!(not_judged(&m(unjudged), &Rule::default()).contains("90 s"));
    }

    #[test]
    fn swapped_exchanges_the_two_runs() {
        let measures = m(|r, _| r.gain_db = 10.5);
        let back = swapped(&measures).expect("the measures carry a reverse block");
        let run = back.run().expect("the reverse run had no error");
        assert_eq!((run.dc, run.ag, run.gap_s, run.ls_db), (0.06, 0.31, 470.0, 17.1));
        let Reverse::Block(block) = &run.rev else { panic!("swapped gives a reverse block") };
        assert_eq!((block.rev_dc, block.rev_ag, block.rev_tenths_comm), (0.08, 0.54, Some(9)));
        // the programme against the commentary as reference
        assert_eq!(verdict(&back, &Rule::default()), Verdict::ReferenceUnclear);
        assert_eq!(swapped(&Measures::Run(pans())), None);
    }

    #[test]
    fn the_programme_against_a_commentary_taken_as_the_reference_is_kept_and_flagged() {
        let m = analyse(comment(), mix(), None, true, &Constants::default());
        assert!(reverse_pattern(&m, &Rule::default()));
        assert_eq!(verdict(&m, &Rule::default()), Verdict::ReferenceUnclear);
    }

    #[test]
    fn a_commentary_that_pauses_over_a_film_is_not_blocked_by_its_pauses() {
        let (film_mix, film_centre) = film(900, 3);
        let mut rnd = PyRandom::new(10);
        let mut voice: Vec<f64> = Vec::new();
        // one speaker: 2.8 s of talk, 1.2 s of pause, the film 26 dB down underneath
        // (int(2.8 * FPS) is 140 and int(1.2 * FPS) is 60 in Python; the casts truncate the same products)
        while voice.len() < film_mix.len() {
            voice.extend(speech(&mut rnd, (2.8 * FPS as f64) as usize, -18.0, -27.0));
            voice.extend(std::iter::repeat(-100.0).take((1.2 * FPS as f64) as usize));
        }
        let cand: Vec<f64> = voice.iter().zip(&film_mix).map(|(&v, &x)| power_sum(v, x - 26.0)).collect();
        let m = analyse(&film_mix, &cand, Some(film_centre.as_slice()), true, &Constants::default());
        let Reverse::Block(block) = &m.run().expect("measured").rev else { panic!("analyse gives a reverse block") };
        // the earlier reverse test would block it
        assert!(block.rev_gap_s >= 90.0 && block.rev_ag >= 0.30);
        assert!(!reverse_pattern(&m, &Rule::default()));
        assert_eq!(verdict(&m, &Rule::default()), Verdict::Commentary);
    }
}

/// New: Rule::NAMES, get and set (spec 3.3: --set for a rule key).
mod rule_set {
    use crate::audio::measures::{Num, NumKind, SetError};
    use crate::audio::rule::Rule;

    #[test]
    fn names_defaults_and_serde_follow_the_python_rule() {
        let expected: [(&str, Num); 20] = [
            ("overlap_min", Num::Float(0.90)),
            ("main_silent_max", Num::Float(0.10)),
            ("gap_s_min", Num::Float(90.0)),
            ("dc_max", Num::Float(0.30)),
            ("ag_min", Num::Float(0.50)),
            ("tenths_min", Num::Int(6)),
            ("tenths_share", Num::Float(0.90)),
            ("smi_min", Num::Float(0.50)),
            ("ls_max", Num::Float(13.0)),
            ("added_s_min", Num::Float(60.0)),
            ("ad_dc_min", Num::Float(0.90)),
            ("ad_adl_max", Num::Float(0.02)),
            ("ad_ag_min", Num::Float(0.15)),
            ("ad_ag_s_min", Num::Float(45.0)),
            ("ad_tenths_share", Num::Float(0.60)),
            ("note_comm_dc_max", Num::Float(0.50)),
            ("note_comm_ag_min", Num::Float(0.30)),
            ("note_ad_dc_min", Num::Float(0.80)),
            ("note_ad_ag_min", Num::Float(0.10)),
            ("note_ad_ag_s_min", Num::Float(30.0)),
        ];
        assert_eq!(Rule::NAMES.map(|(name, _)| name), expected.map(|(name, _)| name));
        let ints: Vec<&str> = Rule::NAMES.iter().filter(|(_, kind)| *kind == NumKind::Int).map(|(name, _)| *name).collect();
        assert_eq!(ints, ["tenths_min"]);
        let rule = Rule::default();
        for (name, value) in expected {
            assert_eq!(rule.get(name), Some(value), "{name}");
        }
        assert_eq!(rule.get("DC_MAX"), None);
        let value = serde_json::to_value(&rule).expect("serialisable");
        let keys: Vec<&str> = value.as_object().expect("an object").keys().map(String::as_str).collect();
        assert_eq!(keys, expected.map(|(name, _)| name));
        assert!(value["tenths_min"].is_i64());
        assert!(value["gap_s_min"].is_f64());
    }

    #[test]
    fn set_matches_a_rule_key_whatever_its_case_and_keeps_a_float_a_float() {
        let mut rule = Rule::default();
        assert_eq!(rule.set("DC_MAX", 0.25), Ok(Num::Float(0.25)));
        assert_eq!(rule.dc_max, 0.25);
        assert_eq!(rule.set("Ls_Max", 12.0), Ok(Num::Float(12.0)));
        assert_eq!(rule.get("ls_max"), Some(Num::Float(12.0)));
    }

    #[test]
    fn set_takes_a_whole_number_for_tenths_min_and_refuses_what_is_not_a_rule_key() {
        let mut rule = Rule::default();
        assert_eq!(rule.set("TENTHS_MIN", 7.0), Ok(Num::Int(7)));
        assert_eq!(rule.tenths_min, 7);
        assert_eq!(rule.set("tenths_min", 6.5), Err(SetError::NotWhole));
        assert_eq!(rule.tenths_min, 7);
        // a measure constant, not a rule key: parse_sets (Task 8) tries the constants next
        assert_eq!(rule.set("SILENT_DB", -80.0), Err(SetError::Unknown));
        assert_eq!(rule.set("dc", 0.1), Err(SetError::Unknown));
        assert_eq!(rule, Rule { tenths_min: 7, ..Rule::default() });
    }
}

/// New: the Python's strings, verbatim.
mod spellings {
    use crate::audio::rule::{meaning, Verdict, VETO_MEANING};

    #[test]
    fn verdicts_are_spelt_as_the_python_writes_them() {
        let names: Vec<&str> = Verdict::ALL.iter().map(|v| v.as_str()).collect();
        assert_eq!(
            names,
            [
                "duplicate",
                "commentary",
                "commentary-cross-language",
                "reference-unclear",
                "ad-pattern",
                "note-commentary",
                "note-ad",
                "keep",
                "not-judged",
            ]
        );
        for v in Verdict::ALL {
            assert_eq!(v.as_str().parse::<Verdict>(), Ok(v));
            assert_eq!(serde_json::to_value(v).expect("serialisable"), serde_json::Value::String(v.as_str().to_string()));
            let back: Verdict = serde_json::from_value(serde_json::Value::String(v.as_str().to_string())).expect("a verdict");
            assert_eq!(back, v);
        }
        assert!("Commentary".parse::<Verdict>().is_err());
        assert!("commentary_cross_language".parse::<Verdict>().is_err());
    }

    #[test]
    fn meanings_are_the_python_texts() {
        assert_eq!(meaning(Verdict::Duplicate), "exact duplicate (same packets): keep the first copy, drop this one; no audio analysis needed");
        assert_eq!(meaning(Verdict::Commentary), "commentary pattern: kept and flagged for review until experiment 13 answers yes, then dropped");
        assert_eq!(meaning(Verdict::CommentaryCrossLanguage), "commentary pattern on a track in another language than the reference: keep and flag for review (commentary_detected); never dropped by content until real dubs in a wanted language have been measured (EXP-13 addendum 9.2)");
        assert_eq!(meaning(Verdict::ReferenceUnclear), "the main track looks like a commentary over this one (reverse run): one of the two is not what its rank says (a commentary or a stem ranked first, or a stem among the candidates): keep, review");
        assert_eq!(meaning(Verdict::AdPattern), "audio-description pattern: keep, note possible_audio_description (never dropped by content)");
        assert_eq!(meaning(Verdict::NoteCommentary), "keep, note possible_commentary");
        assert_eq!(meaning(Verdict::NoteAd), "keep, note possible_audio_description");
        assert_eq!(meaning(Verdict::Keep), "keep: programme audio, nothing added");
        assert_eq!(meaning(Verdict::NotJudged), "keep: no content verdict");
        assert_eq!(VETO_MEANING, "the position veto holds (row 0, D21): the reference may not be the programme, so no commentary is confirmed on this title: keep, review (audio_reference_unclear)");
    }
}

/// CENTRE.
fn centre() -> &'static [f64] {
    &programme_pair().1
}

/// sim.results(d): RULE, the module constants and analyse (Plain).
fn results_of(tree: &MemTree, folder: &PathKey) -> FolderResults {
    results_with(tree, folder, &Rule::default())
}

/// sim.results(d, rule).
fn results_with(tree: &MemTree, folder: &PathKey, rule: &Rule) -> FolderResults {
    let consts = Constants::default();
    results(tree, folder, rule, &consts, &mut Plain { consts: &consts }).expect("the folder is analysed")
}

/// Py: test_audio_similarity.py, class PositionVeto (4 tests). Row 0 of the rule (owner decision D21): a reference
/// that is not the first stream of its language, when that first stream is lossless or has more than two channels,
/// confirms no commentary on the title.
mod position_veto {
    use std::collections::HashMap;

    use super::{centre, comment, mix};
    use crate::audio::measures::{analyse, Constants, Measures};
    use crate::audio::rule::{
        first_of_language, position_veto, results, AnalyseFn, FolderResults, Measured, Rule, Verdict, Veto,
    };
    use crate::recording::files::FfprobeStream;
    use crate::recording::{MemTree, Ordered, PathKey};
    use crate::testsupport::level_file_text;

    /// A stream of these tests: (index, codec_name, profile, channels, language).
    type Stream<'a> = (i64, &'a str, Option<&'a str>, i64, &'a str);

    /// The ffprobe stream list PositionVeto.veto and streams_json build: a video stream 0, then the audio streams.
    fn stream_list(streams: &[Stream]) -> Vec<FfprobeStream> {
        let mut list = vec![FfprobeStream {
            index: 0,
            codec_type: Some("video".to_string()),
            codec_name: None,
            profile: None,
            channels: None,
            channel_layout: None,
            tags: None,
        }];
        for &(index, codec, profile, channels, language) in streams {
            list.push(FfprobeStream {
                index,
                codec_type: Some("audio".to_string()),
                codec_name: Some(codec.to_string()),
                profile: profile.map(str::to_string),
                channels: Some(channels),
                channel_layout: None,
                tags: Some(Ordered(vec![("language".to_string(), language.to_string())])),
            });
        }
        list
    }

    /// PositionVeto.veto: sim.position_veto(<that list>, main).
    fn veto(streams: &[Stream], main: i64) -> Veto {
        position_veto(Some(stream_list(streams).as_slice()), main)
    }

    /// streams_json(d, streams): the folder's streams.json as audio-levels.ps1 writes it (ffprobe's stream list).
    fn streams_json(tree: &mut MemTree, dir: &str, streams: &[Stream]) {
        let mut list = vec![serde_json::json!({"index": 0, "codec_type": "video"})];
        for &(index, codec, profile, channels, language) in streams {
            list.push(serde_json::json!({
                "index": index, "codec_type": "audio", "codec_name": codec, "profile": profile, "channels": channels,
                "tags": {"language": language}
            }));
        }
        tree.insert_file(&format!("{dir}/streams.json"), serde_json::json!({"streams": list}).to_string());
    }

    /// The Python test's analyse_fn: analyse with a cache keyed by (len(main), round(sum(cand), 1), dialogue is
    /// None), so that the second results() reuses the first one's measures. The key holds the exact bits of
    /// sum(cand) in place of round(..., 1) (py_round is Task 10's): it only has to tell the three candidates apart,
    /// and the same series always gives the same sum.
    struct Cache {
        measured: HashMap<(usize, u64, bool), Measures>,
    }

    impl AnalyseFn for Cache {
        fn measure(&mut self, main: &[f64], cand: &[f64], dialogue: Option<&[f64]>) -> Measured {
            let key = (main.len(), cand.iter().sum::<f64>().to_bits(), dialogue.is_none());
            let measures = self
                .measured
                .entry(key)
                .or_insert_with(|| analyse(main, cand, dialogue, true, &Constants::default()))
                .clone();
            Measured { measures, key: None }
        }
    }

    #[test]
    fn it_holds_when_a_lossless_first_stream_is_not_the_reference() {
        // 8.6 ranks the 5.1 first
        let v = veto(&[(1, "truehd", None, 2, "eng"), (2, "ac3", None, 6, "eng")], 2);
        assert_eq!(v.holds, Some(true));
        assert_eq!(
            v.why,
            "the reference, stream 2 (ac3 6 ch), is not the first eng stream of the file: stream 1 (truehd 2 ch) comes first and is lossless or has more than two channels, so the reference choice may be inverted"
        );
    }

    #[test]
    fn it_holds_when_a_multichannel_lossy_first_stream_is_not_the_reference() {
        let v = veto(&[(1, "ac3", None, 6, "eng"), (2, "dts", Some("DTS-HD MA"), 6, "eng")], 2);
        assert_eq!(v.holds, Some(true));
        assert!(v.why.contains("stream 1 (ac3 6 ch) comes first"), "{}", v.why);
    }

    #[test]
    fn it_does_not_hold_otherwise() {
        assert_eq!(
            veto(&[(1, "ac3", None, 2, "eng"), (2, "dts", Some("DTS"), 6, "eng")], 2),
            Veto {
                holds: Some(false),
                why: "the reference, stream 2 (dts DTS 6 ch), is not the first eng stream of the file, but stream 1 (ac3 2 ch), which comes first, is lossy with two channels or fewer".to_string(),
            }
        );
        assert_eq!(
            veto(&[(1, "dts", Some("DTS-HD MA"), 6, "eng"), (2, "ac3", None, 2, "eng")], 1),
            Veto {
                holds: Some(false),
                why: "the reference, stream 1 (dts DTS-HD MA 6 ch), is the first eng stream of the file".to_string(),
            }
        );
        assert_eq!(veto(&[(1, "ac3", None, 2, "eng")], 1).holds, Some(false)); // a single stream
        assert_eq!(veto(&[(1, "truehd", None, 8, "fra"), (2, "ac3", None, 6, "spa")], 2).holds, Some(false)); // other language
        assert_eq!(
            position_veto(None, 1),
            Veto { holds: None, why: "not checked: the folder has no streams.json".to_string() }
        );
        assert_eq!(veto(&[(2, "ac3", None, 6, "eng")], 5).holds, None);
        assert_eq!(first_of_language(&[], 1), None);
    }

    #[test]
    fn it_never_turns_a_keep_into_a_drop() {
        // a quieter, 60 ms late copy of the programme
        let copy: Vec<f64> = [-100.0; 3].iter().copied().chain(mix()[..mix().len() - 3].iter().map(|v| v - 6.0)).collect();
        let mut tree = MemTree::new(".", "lazymkv-test");
        let dir = "audio-levels-20ms-2-1-3-4";
        tree.insert_file(&format!("{dir}/main-mix.txt"), level_file_text(mix()));
        tree.insert_file(&format!("{dir}/main-fc.txt"), level_file_text(centre()));
        for (n, values) in [(1, &copy[..]), (3, comment()), (4, mix())] {
            tree.insert_file(&format!("{dir}/cand-{n}.txt"), level_file_text(values));
        }
        tree.insert_file(
            &format!("{dir}/manifest.txt"),
            concat!(
                "# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n",
                "main=2 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc=yes frames=1 md5=aa\n",
                "cand=1 codec=dts profile=DTS channels=6 layout=5.1(side) language=eng frames=1 md5=bb\n",
                "cand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5=cc\n",
                "cand=4 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng frames=1 md5=aa\n"
            ),
        );
        let d = PathKey::new(dir);
        let mut cache = Cache { measured: HashMap::new() };
        // no streams.json: the veto is not checked
        let free = results(&tree, &d, &Rule::default(), &Constants::default(), &mut cache).expect("analysed");
        streams_json(
            &mut tree,
            dir,
            &[
                (1, "dts", Some("DTS"), 6, "eng"),
                (2, "dts", Some("DTS-HD MA"), 6, "eng"),
                (3, "ac3", None, 2, "eng"),
                (4, "dts", Some("DTS-HD MA"), 6, "eng"),
            ],
        );
        let vetoed = results(&tree, &d, &Rule::default(), &Constants::default(), &mut cache).expect("analysed");
        let verdicts = |res: &FolderResults| res.candidates.iter().map(|c| c.verdict).collect::<Vec<_>>();
        assert_eq!(free.veto.holds, None);
        assert_eq!(verdicts(&free), [Verdict::Keep, Verdict::Commentary, Verdict::Duplicate]);
        assert_eq!(vetoed.veto.holds, Some(true));
        assert_eq!(verdicts(&vetoed), [Verdict::ReferenceUnclear, Verdict::ReferenceUnclear, Verdict::Duplicate]);
        assert_eq!(
            vetoed.candidates.iter().map(|c| c.before_veto).collect::<Vec<_>>(),
            [Some(Verdict::Keep), Some(Verdict::Commentary), None]
        );
        // The report's "  veto: holds (row 0, D21): ..." line and the "verdict: reference-unclear (<VETO_MEANING>;
        // rows 3 to 9 alone: commentary)" line (Task 6) print the veto's why and the before_veto checked above.
        assert!(
            vetoed.veto.why.starts_with(
                "the reference, stream 2 (dts DTS-HD MA 6 ch), is not the first eng stream of the file: stream 1 (dts DTS 6 ch) comes first"
            ),
            "{}",
            vetoed.veto.why
        );
    }
}

/// Py: test_audio_similarity.py, class CrossLanguage (6 tests).
mod cross_language {
    use super::{centre, comment, mix, results_of};
    use crate::audio::levels::Fields;
    use crate::audio::measures::{analyse, Constants};
    use crate::audio::rule::{cross_language, verdict, Rule, Verdict};
    use crate::lang::{iso639, ISO639, TMDB_NO_LANGUAGE};
    use crate::recording::{py_fixed, MemTree, Ordered, PathKey};
    use crate::testsupport::{level_file_text, manifest, speech, PyRandom};

    /// A candidate of CrossLanguage.folder: (stream, levels, channels, language, codec, profile). The Python's
    /// defaults (language eng, codec ac3, profile "-") are written out at every call.
    type Cand<'a> = (i64, &'a [f64], i64, &'a str, &'a str, &'a str);

    /// CrossLanguage.folder: a Spanish film's level folder (main stream 1, DTS-HD MA 5.1 with its centre channel)
    /// with the candidates given.
    fn folder(tree: &mut MemTree, cands: &[Cand]) -> PathKey {
        let numbers: Vec<String> = cands.iter().map(|c| c.0.to_string()).collect();
        let dir = format!("audio-levels-20ms-1-{}", numbers.join("-"));
        tree.insert_file(&format!("{dir}/main-mix.txt"), level_file_text(mix()));
        tree.insert_file(&format!("{dir}/main-fc.txt"), level_file_text(centre()));
        let mut lines = vec![
            "# lazymkv audio-levels v2 (experiment 13)".to_string(),
            "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400".to_string(),
            "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=spa fc=yes frames=1 md5=aa".to_string(),
        ];
        for &(n, values, ch, lang, codec, profile) in cands {
            tree.insert_file(&format!("{dir}/cand-{n}.txt"), level_file_text(values));
            lines.push(format!(
                "cand={n} codec={codec} profile={profile} channels={ch} layout=x language={lang} frames=1 md5=m{n}"
            ));
        }
        tree.insert_file(&format!("{dir}/manifest.txt"), lines.join("\n") + "\n");
        PathKey::new(&dir)
    }

    /// A manifest line's fields, in order.
    fn fields(pairs: &[(&str, &str)]) -> Fields {
        Ordered(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    #[test]
    fn a_commentary_in_another_language_is_judged_against_the_programme_reference() {
        let mut tree = MemTree::new(".", "lazymkv-test");
        let dir = "audio-levels-20ms-1-3";
        tree.insert_file(&format!("{dir}/main-mix.txt"), level_file_text(mix()));
        tree.insert_file(&format!("{dir}/main-fc.txt"), level_file_text(centre()));
        tree.insert_file(&format!("{dir}/cand-3.txt"), level_file_text(comment()));
        tree.insert_file(
            &format!("{dir}/manifest.txt"),
            manifest("yes", "aa", "bb").replace("layout=5.1(side) language=eng", "layout=5.1(side) language=spa"),
        );
        let res = results_of(&tree, &PathKey::new(dir));
        let c = &res.candidates[0];
        // never a drop verdict (addendum 9.2)
        assert_eq!((c.cross_language.as_str(), c.verdict), ("eng against spa", Verdict::CommentaryCrossLanguage));
        // no other English stream: nothing to compare it with
        assert_eq!(c.peer, None);
        // The report's line "stream 3: ac3 2 ch stereo eng (cross-language: eng against spa)" (Task 6) prints these
        // manifest fields and the cross_language checked above.
        assert_eq!(
            ["codec", "channels", "layout", "language"].map(|k| c.info.get(k).map(String::as_str)),
            [Some("ac3"), Some("2"), Some("stereo"), Some("eng")]
        );
        assert_eq!(cross_language(&fields(&[("language", "spa")]), &fields(&[("language", "und")])), "");
    }

    #[test]
    fn a_companion_of_a_dub_follows_the_dub_and_gets_only_a_note() {
        // The dub stand-in has the commentary pattern against the original, as a loosely timed dub can (addendum
        // 9.2); its 2.0 companion is the same mix 6 dB down and 60 ms late.
        let dub = comment();
        let companion: Vec<f64> = [-100.0; 3].iter().copied().chain(dub[..dub.len() - 3].iter().map(|v| v - 6.0)).collect();
        let mut tree = MemTree::new(".", "lazymkv-test");
        let d = folder(&mut tree, &[(2, dub, 6, "eng", "ac3", "-"), (3, &companion[..], 2, "eng", "ac3", "-")]);
        let res = results_of(&tree, &d);
        let [first, second] = &res.candidates[..] else { panic!("two candidates") };
        // the first English stream
        assert_eq!((first.verdict, &first.peer), (Verdict::CommentaryCrossLanguage, &None));
        let peer = second.peer.as_ref().expect("a peer run");
        assert_eq!((peer.stream.as_str(), peer.verdict), ("2", Verdict::Keep));
        let dc = peer.measures.run().expect("measured").dc;
        assert!(dc > 0.9, "{dc}");
        assert_eq!(second.verdict, Verdict::NoteCommentary);
        // The report's line "    peer: against stream 2 (placed first in the candidate's language): DC 1.00 ...:
        // keep, so not confirmed" (Task 6) prints fmt(dc) and the peer verdict checked above.
        assert_eq!(py_fixed(dc, 2), "1.00");
    }

    #[test]
    fn a_commentary_beside_a_dub_keeps_the_cross_language_verdict() {
        // a dub that is the programme
        let shifted: Vec<f64> = mix().iter().map(|v| v - 2.0).collect();
        let mut tree = MemTree::new(".", "lazymkv-test");
        let d = folder(&mut tree, &[(2, &shifted[..], 6, "eng", "ac3", "-"), (3, comment(), 2, "eng", "ac3", "-")]);
        let res = results_of(&tree, &d);
        let [dub, comm] = &res.candidates[..] else { panic!("two candidates") };
        assert_eq!(dub.verdict, Verdict::Keep);
        let peer = comm.peer.as_ref().expect("a peer run");
        assert_eq!((peer.stream.as_str(), peer.verdict), ("2", Verdict::Commentary));
        assert_eq!(comm.verdict, Verdict::CommentaryCrossLanguage);
    }

    #[test]
    fn the_peer_is_the_first_stream_of_the_candidates_own_language_as_8_6_ranks_it() {
        let shifted: Vec<f64> = mix().iter().map(|v| v - 2.0).collect();
        let mut tree = MemTree::new(".", "lazymkv-test");
        // a Spanish core (6 channels) beside the English commentary: not the commentary's peer
        let d = folder(&mut tree, &[(2, &shifted[..], 6, "spa", "ac3", "-"), (3, comment(), 2, "eng", "ac3", "-")]);
        let res = results_of(&tree, &d);
        let [core, comm] = &res.candidates[..] else { panic!("two candidates") };
        assert_eq!((core.verdict, comm.verdict, &comm.peer), (Verdict::Keep, Verdict::CommentaryCrossLanguage, &None));
        // two English 5.1 streams: the lossless one (the manifest writes DTS-HD_MA) is placed first
        let d = folder(&mut tree, &[(4, comment(), 6, "eng", "ac3", "-"), (5, mix(), 6, "eng", "dts", "DTS-HD_MA")]);
        let res = results_of(&tree, &d);
        let [comm, _dub] = &res.candidates[..] else { panic!("two candidates") };
        assert_eq!(comm.peer.as_ref().map(|peer| peer.stream.as_str()), Some("5"));
    }

    #[test]
    fn the_language_table_is_iso_639_and_tmdbs_own_codes() {
        assert_eq!(
            (iso639("ml"), iso639("bo"), iso639("es")),
            (Some(&["mal"][..]), Some(&["bod", "tib"][..]), Some(&["spa"][..]))
        );
        assert!(iso639("cn").expect("cn").contains(&"yue"));
        assert!(iso639("sh").expect("sh").contains(&"hrv"));
        assert!(iso639(TMDB_NO_LANGUAGE).is_none());
        assert!(ISO639.len() >= 185, "{}", ISO639.len());
        for (_, codes) in ISO639 {
            // Python: codes and all(len(c) == 3 and c.isalpha() and c.islower() for c in codes). len counts code
            // points; isalpha is every character a letter; islower is every cased character lower case and at least
            // one cased character (no helper for it exists in core; the codes are ASCII).
            let good = !codes.is_empty()
                && codes.iter().all(|c| {
                    c.chars().count() == 3
                        && c.chars().all(char::is_alphabetic)
                        && c.chars().any(char::is_lowercase)
                        && !c.chars().any(char::is_uppercase)
                });
            assert!(good, "{codes:?}");
        }
    }

    #[test]
    fn a_dub_is_kept_by_where_its_speech_sits_not_by_dc() {
        let mut rnd = PyRandom::new(11);
        // other words, in the programme's dialogue windows only
        let other = speech(&mut rnd, mix().len(), -20.0, -30.0);
        let dub: Vec<f64> = other
            .iter()
            .zip(mix())
            .zip(centre())
            .map(|((&o, &x), &c)| if c > -50.0 { o } else { x })
            .collect();
        let m = analyse(mix(), &dub, Some(centre()), true, &Constants::default());
        let run = m.run().expect("measured");
        assert!(run.dc < 0.9, "{}", run.dc);
        assert!(run.ag < 0.10, "{}", run.ag);
        assert_eq!(verdict(&m, &Rule::default()), Verdict::Keep);
    }
}

/// Py: test_audio_dataset.py, class Similarity (1 test).
mod similarity {
    use super::{centre, comment, mix, results_of, results_with};
    use crate::audio::levels::refusal;
    use crate::audio::measures::Constants;
    use crate::audio::rule::{Dialogue, Rule, Verdict};
    use crate::recording::{MemTree, PathKey};
    use crate::testsupport::level_file_text;

    /// test_audio_dataset.folder(label_dir, main_md5, cands, fc) with its other arguments at their defaults
    /// (channels 6, main 1, streams None: the default streams.json, langs None: every stream English);
    /// cands are (stream, levels, md5).
    fn folder(tree: &mut MemTree, label_dir: &str, main_md5: &str, cands: &[(i64, &[f64], &str)], fc: bool) -> PathKey {
        let (main, channels) = (1_i64, 6_i64);
        let mut numbers = vec![main.to_string()];
        numbers.extend(cands.iter().map(|c| c.0.to_string()));
        let dir = format!("{label_dir}/audio-levels-20ms-{}", numbers.join("-"));
        tree.insert_file(&format!("{dir}/main-mix.txt"), level_file_text(mix()));
        if fc {
            tree.insert_file(&format!("{dir}/main-fc.txt"), level_file_text(centre()));
        }
        let mut lines = vec![
            "# lazymkv audio-levels v2 (experiment 13)".to_string(),
            "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400".to_string(),
            format!(
                "main={main} codec=dts profile=- channels={channels} layout={} language=eng fc={} frames={} md5={main_md5}",
                if fc { "5.1(side)" } else { "stereo" },
                if fc { "yes" } else { "no" },
                mix().len()
            ),
        ];
        for &(n, values, md5) in cands {
            tree.insert_file(&format!("{dir}/cand-{n}.txt"), level_file_text(values));
            lines.push(format!(
                "cand={n} codec=ac3 profile=- channels=2 layout=stereo language=eng frames={} md5={md5}",
                values.len()
            ));
        }
        tree.insert_file(&format!("{dir}/manifest.txt"), lines.join("\n") + "\n");
        // every stream up to the highest measured one, English, the main with `channels` and the others 2
        let top = cands.iter().map(|c| c.0).fold(main, i64::max);
        let mut streams = vec![serde_json::json!({"index": 0, "codec_type": "video"})];
        for k in 1..=top {
            let (ch, codec) = if k == main { (channels, "dts") } else { (2, "ac3") };
            streams.push(serde_json::json!({
                "index": k, "codec_type": "audio", "codec_name": codec, "profile": null, "channels": ch,
                "tags": {"language": "eng", "title": format!("t{k}")}
            }));
        }
        tree.insert_file(&format!("{dir}/streams.json"), serde_json::json!({"streams": streams}).to_string());
        PathKey::new(&dir)
    }

    #[test]
    fn results_are_the_report_as_data_and_a_reference_only_folder_is_valid() {
        let mut tree = MemTree::new(".", "lazymkv-test");
        let short = &comment()[..600];
        let d = folder(&mut tree, "t", "m", &[(3, comment(), "c"), (4, mix(), "m"), (5, short, "s")], true);
        let res = results_of(&tree, &d);
        let rows: Vec<(&str, Verdict, Option<&str>, &str)> = res
            .candidates
            .iter()
            .map(|r| (r.stream.as_str(), r.verdict, r.duplicate_of.as_deref(), r.not_judged.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("3", Verdict::Commentary, None, ""),
                ("4", Verdict::Duplicate, Some("1"), ""),
                ("5", Verdict::NotJudged, None, "too short"),
            ]
        );
        assert_eq!(res.dialogue, Dialogue::Fc);
        // The report's "verdict: commentary" line (Task 6) prints the first candidate's verdict checked above.
        let strict = Rule { dc_max: 0.0, ..Rule::default() };
        assert_eq!(results_with(&tree, &d, &strict).candidates[0].verdict, Verdict::NoteCommentary);
        let r = folder(&mut tree, "r", "m", &[], false);
        assert_eq!(refusal(&tree, &r), Ok(String::new()));
        // The report's "no candidate stream: nothing to judge" line (Task 6) follows from this empty list.
        assert!(results_of(&tree, &r).candidates.is_empty());
        for (name, _) in Constants::NAMES {
            assert!(Constants::default().get(name).is_some(), "{name}");
        }
    }
}

/// New: results' contract with its callers: the refusal is its error, every pair goes through the hook Task 8's
/// cache plugs into and keeps the hook's key, the centre channel's SILENT_DB test reads the constants given, and Veto
/// and Dialogue serialise as the Python writes them.
mod results_contract {
    use crate::audio::measures::{Constants, Measures, Run};
    use crate::audio::rule::{results, AnalyseFn, Dialogue, Measured, Plain, Rule, Verdict, Veto};
    use crate::recording::{MemTree, PathKey, Tree};
    use crate::testsupport::level_file_text;

    /// A hook that measures nothing: every pair gets the measured commentary of class Rule without a reverse block
    /// (so its verdict is commentary) and the key "key-<n>" for the n-th call; it records (frames of main, frames of
    /// cand, whether a dialogue curve was given).
    struct Stub {
        calls: Vec<(usize, usize, bool)>,
    }

    impl AnalyseFn for Stub {
        fn measure(&mut self, main: &[f64], cand: &[f64], dialogue: Option<&[f64]>) -> Measured {
            self.calls.push((main.len(), cand.len(), dialogue.is_some()));
            let commentary = Run {
                overlap: 1.0,
                main_silent: 0.0,
                gap_s: 355.0,
                dc: 0.03,
                ag: 0.66,
                ag_s: 234.0,
                adl: 0.18,
                tenths: 10,
                tenths_comm: 10,
                tenths_ad: 10,
                smi: 0.58,
                ls_db: 10.1,
                added_s: 518.0,
                ..Run::blank()
            };
            Measured { measures: Measures::Run(commentary), key: Some(format!("key-{}", self.calls.len())) }
        }
    }

    /// A level folder of constant levels: main stream 1 in `main_lang` (1200 frames at -30 dB), main-fc.txt at
    /// `fc_db` when given, and English candidates (stream, frames, channels) at -30 dB.
    fn constant_folder(tree: &mut MemTree, dir: &str, main_lang: &str, fc_db: Option<f64>, cands: &[(i64, usize, i64)]) -> PathKey {
        tree.insert_file(&format!("{dir}/main-mix.txt"), level_file_text(&vec![-30.0; 1200]));
        if let Some(db) = fc_db {
            tree.insert_file(&format!("{dir}/main-fc.txt"), level_file_text(&vec![db; 1200]));
        }
        let mut lines = vec![
            "# lazymkv audio-levels v2 (experiment 13)".to_string(),
            "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400".to_string(),
            format!(
                "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language={main_lang} fc={} frames=1 md5=aa",
                if fc_db.is_some() { "yes" } else { "no" }
            ),
        ];
        for &(n, frames, ch) in cands {
            tree.insert_file(&format!("{dir}/cand-{n}.txt"), level_file_text(&vec![-30.0; frames]));
            lines.push(format!("cand={n} codec=ac3 profile=- channels={ch} layout=x language=eng frames=1 md5=m{n}"));
        }
        tree.insert_file(&format!("{dir}/manifest.txt"), lines.join("\n") + "\n");
        PathKey::new(dir)
    }

    #[test]
    fn the_hook_measures_every_pair_and_its_keys_are_kept() {
        let mut tree = MemTree::new(".", "lazymkv-test");
        let d = constant_folder(&mut tree, "audio-levels-20ms-1-2-3", "spa", Some(-20.0), &[(2, 1100, 6), (3, 1000, 2)]);
        let mut stub = Stub { calls: Vec::new() };
        let res = results(&tree, &d, &Rule::default(), &Constants::default(), &mut stub).expect("analysed");
        // the forward runs against main-mix with main-fc as dialogue, then the peer run of stream 3 against stream 2
        // (stream 2 is placed first among the English candidates: 6 channels against 2)
        assert_eq!(stub.calls, [(1200, 1100, true), (1200, 1000, true), (1100, 1000, false)]);
        let [two, three] = &res.candidates[..] else { panic!("two candidates") };
        assert_eq!(
            (two.verdict, two.key.as_deref(), two.peer.is_none(), two.cross_language.as_str()),
            (Verdict::CommentaryCrossLanguage, Some("key-1"), true, "eng against spa")
        );
        assert_eq!((three.verdict, three.key.as_deref()), (Verdict::CommentaryCrossLanguage, Some("key-2")));
        let peer = three.peer.as_ref().expect("a peer run");
        assert_eq!((peer.stream.as_str(), peer.verdict, peer.key.as_deref()), ("2", Verdict::Commentary, Some("key-3")));
        assert_eq!((res.dialogue, res.frames), (Dialogue::Fc, 1200));
        assert_eq!(res.veto, Veto { holds: None, why: "not checked: the folder has no streams.json".to_string() });
        assert_eq!(res.folder, tree.display(&d));
        assert_eq!(res.main.get("main").map(String::as_str), Some("1"));
    }

    #[test]
    fn a_centre_channel_at_or_below_silent_db_is_not_the_dialogue_curve() {
        let mut tree = MemTree::new(".", "lazymkv-test");
        let d = constant_folder(&mut tree, "audio-levels-20ms-1-2", "eng", Some(-100.0), &[(2, 1100, 2)]);
        let mut stub = Stub { calls: Vec::new() };
        let res = results(&tree, &d, &Rule::default(), &Constants::default(), &mut stub).expect("analysed");
        assert_eq!((res.dialogue, stub.calls), (Dialogue::Mix, vec![(1200, 1100, false)]));
        // the same centre channel is the dialogue curve when SILENT_DB is set lower (a --set trial)
        let consts = Constants { SILENT_DB: -110.0, ..Constants::default() };
        let mut stub = Stub { calls: Vec::new() };
        let res = results(&tree, &d, &Rule::default(), &consts, &mut stub).expect("analysed");
        assert_eq!((res.dialogue, stub.calls), (Dialogue::Fc, vec![(1200, 1100, true)]));
    }

    #[test]
    fn a_refused_folder_is_the_error_and_nothing_is_measured() {
        let mut tree = MemTree::new(".", "lazymkv-test");
        tree.insert_dir("audio-levels-20ms-5-6");
        let d = PathKey::new("audio-levels-20ms-5-6");
        let mut stub = Stub { calls: Vec::new() };
        let err = results(&tree, &d, &Rule::default(), &Constants::default(), &mut stub).expect_err("refused");
        assert_eq!(
            err,
            format!(
                "{}: no manifest.txt, so it was not written by the new audio-levels.ps1 (the first measurement's folders, audio-levels-<main>-<cand>, hold 100 ms full-band levels); this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>",
                tree.display(&d)
            )
        );
        assert!(stub.calls.is_empty());
    }

    #[test]
    fn veto_dialogue_and_plain_are_what_the_python_writes() {
        let veto = Veto { holds: None, why: "not checked".to_string() };
        assert_eq!(serde_json::to_string(&veto).expect("serialisable"), r#"{"holds":null,"why":"not checked"}"#);
        let holds = Veto { holds: Some(true), why: String::new() };
        assert_eq!(serde_json::to_string(&holds).expect("serialisable"), r#"{"holds":true,"why":""}"#);
        assert_eq!(serde_json::to_string(&Dialogue::Fc).expect("serialisable"), r#""fc""#);
        assert_eq!(serde_json::to_string(&Dialogue::Mix).expect("serialisable"), r#""mix""#);
        // Plain is analyse with the reverse run and no cache key
        let consts = Constants::default();
        let short = vec![-30.0; 500];
        let measured = Plain { consts: &consts }.measure(&short, &short, None);
        assert_eq!(measured.key, None);
        assert_eq!(measured.measures.error(), Some("too short"));
    }
}
