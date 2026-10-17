//! Row 14 read from synthetic spec text (the design spec's own row is read by crates/dev/tests/retune_subtitles.rs,
//! since core's tests read no repository document), the costly verdicts, and the harness over synthetic
//! recordings in memory.

mod requirements {
    use super::super::{exp14_requirements, Row14};

    /// A section 17 table: row 13 (which also names a fitting disc and must not be read), a row 14 with number words,
    /// counts other than the design spec's and three fitting discs, and row 15.
    const SPEC: &str = "| # | Question | If the answer is no | Result |\n\
| 13 | at least 8 commentaries; Every threshold was fitted on two tracks (`the-office-2005-s3d3-bluray`: the S03E16 commentary), so no track of that disc counts. | fallback | result |\n\
| 14 | (a) Before (a) may be yes, the labelled tracks must include: at least three titles from at least two discs (at least one feature film and two TV episodes) that each have both a regular and an SDH English track; at least 5 SDH and 4 regular English tracks in all; at least 2 real forced English tracks with events; at least 3 commentary subtitle tracks; at least 2 DVDs with English VobSub subtitles; at least 0 UK-style SDH tracks if the owner has a UK disc. Every threshold was fitted on `disc-x` (E1) and `disc-y` and `disc-z`, so none of their tracks counts towards any total above. | fallback | result |\n\
| 15 | later | | |\n";

    #[test]
    fn row_14_is_read_with_its_number_words_and_every_fitting_disc() {
        assert_eq!(
            exp14_requirements(SPEC),
            Ok(Row14 {
                pair_titles: 3,
                pair_discs: 2,
                pair_features: 1,
                pair_tv: 2,
                sdh: 5,
                regular: 4,
                forced: 2,
                commentary: 3,
                dvd_vobsub: 2,
                uk_sdh: 0,
                fitting_discs: vec!["disc-x".to_string(), "disc-y".to_string(), "disc-z".to_string()],
            })
        );
    }

    #[test]
    fn a_spec_without_row_14_is_refused() {
        assert_eq!(
            exp14_requirements("| 13 | a row |\n| 15 | another |\n"),
            Err("spec section 17 has no row starting '| 14 |'".to_string())
        );
    }

    #[test]
    fn a_reworded_row_14_names_the_phrase_it_no_longer_finds() {
        let reworded = SPEC.replace("regular English tracks in all", "plain English tracks in all");
        assert_eq!(
            exp14_requirements(&reworded),
            Err("spec section 17 row 14 no longer contains the phrase /at least \
                 ([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) SDH and \
                 ([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) regular English tracks in all/; update \
                 core::subtitle::labels"
                .to_string())
        );
        let reworded = SPEC.replace("UK-style SDH tracks", "British SDH tracks");
        assert_eq!(
            exp14_requirements(&reworded),
            Err("spec section 17 row 14 no longer contains the phrase /at least \
                 ([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) UK-style SDH tracks?/; update \
                 core::subtitle::labels"
                .to_string())
        );
    }

    #[test]
    fn a_row_14_that_names_no_fitting_disc_is_refused() {
        let refused = Err("spec section 17 row 14 no longer names the discs the thresholds were fitted on".to_string());
        let unnamed = SPEC.replace(
            "Every threshold was fitted on `disc-x` (E1) and `disc-y` and `disc-z`, so none",
            "The thresholds are provisional, so none",
        );
        assert_eq!(exp14_requirements(&unnamed), refused);
        let unquoted = SPEC.replace("`disc-x` (E1) and `disc-y` and `disc-z`", "two discs");
        assert_eq!(exp14_requirements(&unquoted), refused);
    }

    #[test]
    fn a_count_that_is_no_number_is_refused() {
        let huge = SPEC.replace("at least 5 SDH", "at least 99999999999 SDH");
        assert_eq!(exp14_requirements(&huge), Err("spec section 17 row 14: 99999999999 is not a count".to_string()));
    }
}

mod verdicts {
    use crate::subtitle::labels::SubtitleKind;
    use crate::tracks::SubtitleRole;

    use super::super::{costly, Costly, Outcome};

    /// An outcome: `costly` reads only the role and the review.
    fn placed(role: Option<SubtitleRole>, review: bool) -> Outcome {
        Outcome { role, review }
    }

    /// Row 14's dialogue kinds.
    const DIALOGUE: [SubtitleKind; 5] = [
        SubtitleKind::Regular,
        SubtitleKind::Sdh,
        SubtitleKind::Translation,
        SubtitleKind::DubTranscript,
        SubtitleKind::ClosedCaptions,
    ];

    #[test]
    fn a_dialogue_track_dropped_is_costly_with_or_without_a_review() {
        for kind in DIALOGUE {
            for review in [false, true] {
                assert_eq!(costly(kind, &placed(None, review)), Some(Costly::DialogueDropped), "{kind:?} {review}");
            }
        }
        let not_dialogue =
            [SubtitleKind::Forced, SubtitleKind::Commentary, SubtitleKind::Trivia, SubtitleKind::Lyrics, SubtitleKind::Other];
        for kind in not_dialogue {
            for review in [false, true] {
                assert_eq!(costly(kind, &placed(None, review)), None, "{kind:?} {review}");
            }
        }
    }

    #[test]
    fn an_sdh_track_placed_as_regular_without_review_is_costly() {
        assert_eq!(costly(SubtitleKind::Sdh, &placed(Some(SubtitleRole::Regular), false)), Some(Costly::SdhAsRegular));
        assert_eq!(costly(SubtitleKind::Sdh, &placed(Some(SubtitleRole::Regular), true)), None);
        assert_eq!(costly(SubtitleKind::Sdh, &placed(Some(SubtitleRole::Sdh), false)), None);
    }

    #[test]
    fn a_regular_track_placed_as_sdh_without_review_is_costly() {
        assert_eq!(costly(SubtitleKind::Regular, &placed(Some(SubtitleRole::Sdh), false)), Some(Costly::RegularAsSdh));
        assert_eq!(costly(SubtitleKind::Regular, &placed(Some(SubtitleRole::Sdh), true)), None);
        assert_eq!(costly(SubtitleKind::Regular, &placed(Some(SubtitleRole::Regular), false)), None);
    }

    #[test]
    fn a_track_flagged_forced_without_review_that_the_owner_does_not_name_forced_is_costly() {
        for kind in SubtitleKind::ALL.into_iter().filter(|kind| *kind != SubtitleKind::Forced) {
            assert_eq!(costly(kind, &placed(Some(SubtitleRole::Forced), false)), Some(Costly::ForcedWrongly), "{kind:?}");
            assert_eq!(costly(kind, &placed(Some(SubtitleRole::Forced), true)), None, "{kind:?}");
        }
        assert_eq!(costly(SubtitleKind::Forced, &placed(Some(SubtitleRole::Forced), false)), None);
    }

    #[test]
    fn a_forced_track_placed_as_regular_or_sdh_without_review_is_costly() {
        for role in [SubtitleRole::Regular, SubtitleRole::Sdh] {
            assert_eq!(costly(SubtitleKind::Forced, &placed(Some(role), false)), Some(Costly::ForcedAsDialogue), "{role:?}");
            assert_eq!(costly(SubtitleKind::Forced, &placed(Some(role), true)), None, "{role:?}");
        }
    }

    #[test]
    fn a_commentary_trivia_or_lyrics_track_kept_without_review_is_costly() {
        for kind in [SubtitleKind::Commentary, SubtitleKind::Trivia, SubtitleKind::Lyrics] {
            for role in [SubtitleRole::Regular, SubtitleRole::Sdh] {
                assert_eq!(costly(kind, &placed(Some(role), false)), Some(Costly::UnwantedKept), "{kind:?} {role:?}");
                assert_eq!(costly(kind, &placed(Some(role), true)), None, "{kind:?} {role:?}");
            }
            // Kept as forced, the verdict is row 14's earlier one: flagged forced without being forced.
            assert_eq!(costly(kind, &placed(Some(SubtitleRole::Forced), false)), Some(Costly::ForcedWrongly), "{kind:?}");
            assert_eq!(costly(kind, &placed(None, false)), None, "{kind:?}");
        }
    }

    #[test]
    fn every_right_place_and_every_reviewed_place_is_not_costly() {
        let right = [
            (SubtitleKind::Regular, Some(SubtitleRole::Regular)),
            (SubtitleKind::Sdh, Some(SubtitleRole::Sdh)),
            (SubtitleKind::Forced, Some(SubtitleRole::Forced)),
            (SubtitleKind::Forced, None),
            (SubtitleKind::Translation, Some(SubtitleRole::Regular)),
            (SubtitleKind::Translation, Some(SubtitleRole::Sdh)),
            (SubtitleKind::DubTranscript, Some(SubtitleRole::Regular)),
            (SubtitleKind::ClosedCaptions, Some(SubtitleRole::Sdh)),
            (SubtitleKind::ClosedCaptions, Some(SubtitleRole::Regular)),
            (SubtitleKind::Commentary, None),
            (SubtitleKind::Trivia, None),
            (SubtitleKind::Lyrics, None),
            (SubtitleKind::Other, Some(SubtitleRole::Regular)),
            (SubtitleKind::Other, Some(SubtitleRole::Sdh)),
            (SubtitleKind::Other, None),
        ];
        for (kind, role) in right {
            assert_eq!(costly(kind, &placed(role, false)), None, "{kind:?} {role:?}");
        }
        for kind in SubtitleKind::ALL {
            for role in SubtitleRole::ALL {
                assert_eq!(costly(kind, &placed(Some(role), true)), None, "{kind:?} {role:?} with a review");
            }
        }
    }

    #[test]
    fn a_kept_track_with_no_review_is_no_verdict_for_its_kind() {
        assert_eq!(costly(SubtitleKind::Regular, &placed(Some(SubtitleRole::Regular), false)), None);
        assert_eq!(costly(SubtitleKind::Regular, &placed(None, false)), Some(Costly::DialogueDropped));
    }

    #[test]
    fn costly_verdicts_have_the_spellings_and_the_meanings_of_row_14() {
        let spelt: Vec<&str> = Costly::ALL.iter().map(|verdict| verdict.as_str()).collect();
        assert_eq!(
            spelt,
            ["dialogue_dropped", "sdh_as_regular", "regular_as_sdh", "forced_wrongly", "forced_as_dialogue", "unwanted_kept"]
        );
        for verdict in Costly::ALL {
            assert_eq!(serde_json::to_string(&verdict).unwrap(), format!("\"{}\"", verdict.as_str()));
            assert_eq!(verdict.as_str().parse::<Costly>(), Ok(verdict));
        }
        assert_eq!("dropped".parse::<Costly>(), Err("not a costly verdict: dropped".to_string()));
        let meanings: Vec<&str> = Costly::ALL.iter().map(|verdict| verdict.meaning()).collect();
        assert_eq!(
            meanings,
            [
                "a dialogue track dropped, with or without a review",
                "an SDH track placed as regular without review",
                "a regular track placed as SDH without review",
                "a track that is not forced flagged forced without review",
                "a forced track placed as regular or SDH without review",
                "a commentary, trivia or lyrics track kept without review",
            ]
        );
    }
}

mod runs {
    use serde_json::{json, Value};

    use crate::recording::{MemTree, PathKey, Tree};
    use crate::settings::Settings;
    use crate::subtitle::labels::{load, FORMAT};
    use crate::testsupport::scan_text;

    use super::super::{harness, Costly, Row14, SubtitleHarness};

    /// Made-up English lines for the OCR text (no line of any real subtitle): the dialogue of `lazymkv-dev decide`'s
    /// unit test fixture, whose text rule outcomes that test pins.
    const LINES: [&str; 6] = [
        "The cat and the dog went to the shop.",
        "You know that is not what I said.",
        "We have to go back to the house now.",
        "It was the best day of the year.",
        "He said that you would be there.",
        "What do you want me to do with this.",
    ];
    /// A made-up sound cue, put before every third line of a cued stream.
    const CUE: &str = "[DOOR CLOSES]";
    /// MakeMKV's log of a rip: one made-up message, no MSG:4047 removal.
    const RIP: &str = "MSG:5011,0,0,\"made-up line\",\"made-up line\"\n";
    /// The rip summaries' profiles: the eng selection and the all selection.
    const ENG_ONLY: &str = "C:\\made-up\\eng-only.mmcp.xml";
    const ALL_TRACKS: &str = "C:\\made-up\\all-tracks.mmcp.xml";
    /// Title 0's attributes: 2 chapters, 10 minutes, and a playlist on a Blu-ray or a DVD title number on a DVD.
    const BD_TITLE: &[(u32, &str)] = &[(8, "2"), (9, "0:10:00"), (16, "00001.mpls")];
    const DVD_TITLE: &[(u32, &str)] = &[(8, "2"), (9, "0:10:00"), (24, "01")];
    /// Stream attributes as MakeMKV writes them (spec 9.4; the codec names of the recordings).
    const AVC: &[(u32, &str)] = &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")];
    const MPEG2: &[(u32, &str)] = &[(1, "Video"), (5, "V_MPEG2"), (6, "Mpeg2"), (7, "Mpeg2")];
    const AC3: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")];
    const VOBSUB: &[(u32, &str)] = &[(1, "Subtitles"), (3, "eng"), (5, "S_VOBSUB"), (6, ""), (7, "Dvd Subtitles")];
    const CAPTIONS: &[(u32, &str)] = &[
        (1, "Subtitles"),
        (3, "eng"),
        (5, "S_CC608/DVD"),
        (6, "CC"),
        (7, "Closed Captions"),
        (41, "Text"),
        (42, "( Lossy conversion )"),
    ];

    /// A label: (rip label, file track, kind, how, sure).
    type Label = (&'static str, i64, &'static str, &'static str, bool);

    /// One subtitle stream of a synthetic rip: its subs/index.json entry, its packets, and its OCR text when read.
    struct Sub {
        n: i64,
        track: i64,
        language: &'static str,
        codec: &'static str,
        packets: Vec<Value>,
        ocr: Option<String>,
    }

    /// One synthetic recording with one rip label of title 0.
    struct Rec {
        slug: &'static str,
        scan: String,
        tmdb: Option<Value>,
        label: &'static str,
        profile: &'static str,
        tracks: Vec<Value>,
        chapters: i64,
        subs: Vec<Sub>,
        labels: Vec<Label>,
    }

    /// `count` start times from `from`, `step` seconds apart.
    fn starts(count: usize, from: f64, step: f64) -> Vec<f64> {
        (0..count).map(|k| from + step * k as f64).collect()
    }

    /// The OCR text of events at `starts`: one record per event carrying LINES in turn, CUE first on every third
    /// when `cue`.
    fn ocr_text(starts: &[f64], cue: bool) -> String {
        let mut text = String::new();
        for (k, t) in starts.iter().enumerate() {
            let mut lines = vec![LINES[k % 6]];
            if cue && k % 3 == 0 {
                lines.insert(0, CUE);
            }
            text.push_str(&json!({"frame": k, "t": t, "pts": t, "lines": lines}).to_string());
            text.push('\n');
        }
        text
    }

    /// A PGS stream: a 3000-byte packet at each start, a 30-byte clear packet 2 s later.
    fn pgs(n: i64, track: i64, language: &'static str, starts: &[f64], cue: bool, read: bool) -> Sub {
        let packets = starts
            .iter()
            .flat_map(|&t| {
                [
                    json!({"pts_time": format!("{t:.6}"), "duration_time": "2.000000", "size": "3000"}),
                    json!({"pts_time": format!("{:.6}", t + 2.0), "duration_time": "N/A", "size": "30"}),
                ]
            })
            .collect();
        Sub { n, track, language, codec: "HDMV PGS", packets, ocr: read.then(|| ocr_text(starts, cue)) }
    }

    /// An English VobSub stream: a 2000-byte packet at each start, no clear packet.
    fn vobsub(n: i64, track: i64, starts: &[f64], read: bool) -> Sub {
        let packets = starts
            .iter()
            .map(|&t| json!({"pts_time": format!("{t:.6}"), "duration_time": "2.000000", "size": "2000"}))
            .collect();
        Sub { n, track, language: "eng", codec: "VobSub", packets, ocr: read.then(|| ocr_text(starts, false)) }
    }

    /// The DVD's English closed captions as MakeMKV converts them: a text stream, every packet an event, no OCR text.
    fn captions(n: i64, track: i64, starts: &[f64]) -> Sub {
        let packets = starts
            .iter()
            .map(|&t| json!({"pts_time": format!("{t:.6}"), "duration_time": "2.000000", "size": "40"}))
            .collect();
        Sub { n, track, language: "eng", codec: "SubRip/SRT", packets, ocr: None }
    }

    /// mkvmerge -J tracks.
    fn video_track(id: i64, codec: &str) -> Value {
        json!({"id": id, "type": "video", "codec": codec, "properties": {}})
    }
    fn audio_track(id: i64) -> Value {
        json!({"id": id, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}})
    }
    fn subtitle_track(id: i64, codec: &str, language: &str) -> Value {
        json!({"id": id, "type": "subtitles", "codec": codec, "properties": {"language": language}})
    }

    /// A PGS stream's attributes in `language`.
    fn pgs_attrs(language: &'static str) -> Vec<(u32, &'static str)> {
        vec![(1, "Subtitles"), (3, language), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")]
    }

    /// A one-title robot log: the disc's CINFO, title 0's attributes, its streams numbered from 0.
    fn scan_of(disc: &[(u32, &str)], title: &[(u32, &str)], streams: &[Vec<(u32, &'static str)>]) -> String {
        let numbered: Vec<(u32, &[(u32, &str)])> =
            streams.iter().enumerate().map(|(k, attrs)| (k as u32, attrs.as_slice())).collect();
        scan_text(disc, &[(0, title, numbered.as_slice())])
    }

    /// A Blu-ray recording: title 0 with video (scan stream 0, file track 0) and one PGS stream per entry of `subs`
    /// (language, starts, cue, read): scan stream k + 1, file track k + 1, subs/index.json n = k.
    fn bluray(
        slug: &'static str,
        tmdb: Option<Value>,
        label: &'static str,
        profile: &'static str,
        subs: &[(&'static str, Vec<f64>, bool, bool)],
        labels: Vec<Label>,
    ) -> Rec {
        let mut streams = vec![AVC.to_vec()];
        let mut tracks = vec![video_track(0, "AVC/H.264/MPEG-4p10")];
        let mut built = Vec::new();
        for (k, (language, at, cue, read)) in subs.iter().enumerate() {
            let n = k as i64;
            streams.push(pgs_attrs(*language));
            tracks.push(subtitle_track(n + 1, "HDMV PGS", language));
            built.push(pgs(n, n + 1, *language, at, *cue, *read));
        }
        let scan = scan_of(&[(1, "Blu-ray disc")], BD_TITLE, &streams);
        Rec { slug, scan, tmdb, label, profile, tracks, chapters: 2, subs: built, labels }
    }

    /// TMDb's slim record of a film (`title`) and of a show (`name`).
    fn film() -> Option<Value> {
        Some(json!({"title": "Made Up Film", "original_language": "en"}))
    }
    fn show() -> Option<Value> {
        Some(json!({"name": "Made Up Show", "original_language": "en"}))
    }

    /// A film with one English PGS stream (scan stream 1, file track 1) of 40 events, the plain dialogue; read by
    /// OCR when `read`.
    fn disc_a(slug: &'static str, labels: Vec<Label>, read: bool) -> Rec {
        bluray(slug, film(), "eng-only", ENG_ONLY, &[("eng", starts(40, 10.0, 3.0), false, read)], labels)
    }

    /// disc-b, a TV episode: two English PGS streams with the same 40 starts, the plain dialogue (scan stream 1)
    /// and the cued one (scan stream 2, read by OCR when `read_second`).
    fn disc_b(labels: Vec<Label>, read_second: bool) -> Rec {
        let at = starts(40, 10.0, 3.0);
        let subs = [("eng", at.clone(), false, true), ("eng", at, true, read_second)];
        bluray("disc-b", show(), "eng-only", ENG_ONLY, &subs, labels)
    }

    /// disc-d, a DVD: MPEG-2 video (0), English AC-3 audio (1), English VobSub (2, the plain dialogue, read by OCR)
    /// and closed captions (3). The file holds the captions right after the audio (6.2 step 7): file tracks 0
    /// video, 1 audio, 2 captions (n 0), 3 VobSub (n 1).
    fn disc_d(labels: Vec<Label>) -> Rec {
        let at = starts(40, 10.0, 3.0);
        Rec {
            slug: "disc-d",
            scan: scan_of(&[(1, "DVD disc")], DVD_TITLE, &[MPEG2.to_vec(), AC3.to_vec(), VOBSUB.to_vec(), CAPTIONS.to_vec()]),
            tmdb: None,
            label: "all-tracks",
            profile: ALL_TRACKS,
            tracks: vec![
                video_track(0, "MPEG-1/2"),
                audio_track(1),
                subtitle_track(2, "SubRip/SRT", "eng"),
                subtitle_track(3, "VobSub", "eng"),
            ],
            chapters: 2,
            subs: vec![captions(0, 2, &at), vobsub(1, 3, &at, true)],
            labels,
        }
    }

    /// subtitle-labels.json with `labels`, each dated 2026-10-16, given before the verdict was seen
    /// (`saw_verdict` false), with no note and no style (RP25).
    fn labels_json(slug: &str, labels: &[Label]) -> String {
        let tracks: Vec<Value> = labels
            .iter()
            .map(|&(label, stream, kind, how, sure)| {
                json!({"label": label, "stream": stream, "kind": kind, "how": how, "sure": sure,
                       "date": "2026-10-16", "saw_verdict": false, "note": null, "style": null})
            })
            .collect();
        json!({"format": FORMAT, "slug": slug, "tracks": tracks}).to_string()
    }

    /// Rewrites `slug`'s subtitle-labels.json with `labels`, label number `at` carrying each (key, value) of `set`
    /// (RP25's `saw_verdict` and `style`).
    fn relabel(evidence: &mut MemTree, slug: &str, labels: &[Label], at: usize, set: &[(&str, Value)]) {
        let mut file: Value = serde_json::from_str(&labels_json(slug, labels)).expect("labels_json writes JSON");
        for (key, value) in set {
            file["tracks"][at][*key] = value.clone();
        }
        evidence.insert_file(&format!("{slug}/subtitle-labels.json"), file.to_string());
    }

    /// Writes `rec` into the evidence tree and its OCR text into the text tree.
    fn put(rec: &Rec, evidence: &mut MemTree, text: &mut MemTree) {
        let slug = rec.slug;
        evidence.insert_file(&format!("{slug}/scan.txt"), rec.scan.as_str());
        if let Some(tmdb) = &rec.tmdb {
            evidence.insert_file(&format!("{slug}/tmdb.json"), tmdb.to_string());
        }
        evidence.insert_file(&format!("{slug}/subtitle-labels.json"), labels_json(slug, &rec.labels));
        let label = format!("{slug}/{}", rec.label);
        let summary = json!({"ok": true, "title": 0, "profile": rec.profile});
        evidence.insert_file(&format!("{label}/rip-summary.json"), summary.to_string());
        let ident = json!({"tracks": rec.tracks, "chapters": [{"num_entries": rec.chapters}],
                           "container": {"properties": {"duration": 600_000_000_000_i64}}});
        evidence.insert_file(&format!("{label}/mkvmerge.json"), ident.to_string());
        evidence.insert_file(&format!("{label}/rip.txt"), RIP);
        let index: Vec<Value> = rec
            .subs
            .iter()
            .map(|sub| {
                json!({"n": sub.n, "track_id": sub.track, "language": sub.language, "codec": sub.codec,
                       "track_name": null, "forced": false, "default": false})
            })
            .collect();
        evidence.insert_file(&format!("{label}/subs/index.json"), Value::Array(index).to_string());
        for sub in &rec.subs {
            let packets = json!({"packets": sub.packets});
            evidence.insert_file(&format!("{label}/subs/sub-{}.packets.json", sub.n), packets.to_string());
            if let Some(ocr) = &sub.ocr {
                text.insert_file(&format!("{label}/subs-ocr/sub-{}.ocr.jsonl", sub.n), ocr.as_str());
            }
        }
    }

    /// The evidence tree (shown as corpus/owner) and the OCR text tree (shown as text) of `recs`.
    fn trees(recs: &[Rec]) -> (MemTree, MemTree) {
        let mut evidence = MemTree::new("corpus/owner", "owner");
        let mut text = MemTree::new("text", "text");
        for rec in recs {
            put(rec, &mut evidence, &mut text);
        }
        (evidence, text)
    }

    /// The harness over the trees, with or without the OCR text.
    fn judge(evidence: &MemTree, text: Option<&MemTree>, row14: &Row14, settings: &Settings) -> SubtitleHarness {
        harness(evidence, text.map(|tree| tree as &dyn Tree), row14, settings)
    }

    /// Row 14 as the design spec states it (crates/dev/tests/retune_subtitles.rs pins that the spec gives exactly this).
    fn spec_row14() -> Row14 {
        Row14 {
            pair_titles: 3,
            pair_discs: 2,
            pair_features: 1,
            pair_tv: 1,
            sdh: 6,
            regular: 6,
            forced: 1,
            commentary: 1,
            dvd_vobsub: 1,
            uk_sdh: 1,
            fitting_discs: vec!["the-office-2005-s3d3-bluray".to_string(), "pans-labyrinth-2006-uhd".to_string()],
        }
    }

    /// A row 14 that asks for nothing and names no fitting disc.
    fn met_row14() -> Row14 {
        Row14 {
            pair_titles: 0,
            pair_discs: 0,
            pair_features: 0,
            pair_tv: 0,
            sdh: 0,
            regular: 0,
            forced: 0,
            commentary: 0,
            dvd_vobsub: 0,
            uk_sdh: 0,
            fitting_discs: Vec::new(),
        }
    }

    /// The lines of the section whose header starts with `header`, up to the next empty line.
    fn section<'h>(h: &'h SubtitleHarness, header: &str) -> Vec<&'h str> {
        let start = h
            .lines
            .iter()
            .position(|line| line.starts_with(header))
            .unwrap_or_else(|| panic!("no section {header} in {:#?}", h.lines));
        h.lines[start + 1..].iter().take_while(|line| !line.is_empty()).map(String::as_str).collect()
    }

    /// The one line starting with `start`.
    fn line<'h>(h: &'h SubtitleHarness, start: &str) -> &'h str {
        h.lines
            .iter()
            .find(|line| line.starts_with(start))
            .map(String::as_str)
            .unwrap_or_else(|| panic!("no line {start} in {:#?}", h.lines))
    }

    /// The report's last line.
    fn last(h: &SubtitleHarness) -> &str {
        h.lines.last().map(String::as_str).unwrap_or("")
    }

    #[test]
    fn the_report_counts_each_kind_against_row_14_and_exits_1_on_a_costly_verdict() {
        let (evidence, text) = trees(&[
            disc_a("disc-a", vec![("eng-only", 1, "sdh", "read", true)], true),
            disc_b(vec![("eng-only", 1, "regular", "read", true), ("eng-only", 2, "sdh", "watched", true)], true),
            disc_d(vec![("all-tracks", 2, "closed-captions", "read", true), ("all-tracks", 3, "regular", "read", true)]),
        ]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            h.lines,
            [
                "subtitle labels: corpus/owner: 3 recordings with subtitle-labels.json, 5 labelled tracks: 5 judged, 0 not judged, 0 in another language",
                "rule: 8.7 with sub_clear_max_bytes 64, subs_tolerance_ms 300, subs_same_min 0.7, subs_diff_max 0.3; the text rule (EXP-14-design.md section 3) with the same four and its other thresholds as in the code, all provisional",
                "",
                "Tracks (the owner's label; 8.7 as the app decides before experiment 14 answers; the text rule as row 14 (a) judges it):",
                "  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled sdh (read); 8.7 regular; text regular; COSTLY sdh_as_regular",
                "  disc-b/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular (review); text keep (review); reviewed",
                "  disc-b/eng-only stream 2 (scan stream 2, 40 events): labelled sdh (watched); 8.7 sdh (review); text keep (review); reviewed",
                "  disc-d/all-tracks stream 2 (scan stream 3, 40 events): labelled closed-captions (read); 8.7 sdh; text set aside; placed",
                "  disc-d/all-tracks stream 3 (scan stream 2, 40 events): labelled regular (read); 8.7 regular; text regular; placed",
                "",
                "Experiment 14 (spec section 17 row 14 (a); tracks that count: judged here, labelled from text read or watched before the verdict, the owner sure, never on the-office-2005-s3d3-bluray or pans-labyrinth-2006-uhd; the packaging, the menu, a flag, the player and TheDiscDB are observations and do not count on their own, D23):",
                "  regular               2 of 6",
                "  sdh                   2 of 6",
                "  forced                0, with events 0 of 1",
                "  commentary            0 of 1",
                "  trivia                0 (every one the owner has; checked by hand)",
                "  lyrics                0 (every one the owner has; checked by hand)",
                "  translation           0 (every second translation the owner has; checked by hand)",
                "  dub-transcript        0 (every one the owner has; checked by hand)",
                "  closed-captions       1 (with its DVD's VobSub tracks)",
                "  other                 0",
                "  regular and SDH       1 of 3 titles, from 1 of 2 discs; feature films 0 of 1, TV episodes 1 of 1",
                "  DVD VobSub            1 of 1 DVDs with a counted English VobSub track",
                "  UK-style SDH          0 of 1 counted SDH tracks labelled style uk (only if the owner has a UK disc)",
                "  placed, no review     3 of 5 judged tracks (row 14 (a) asks for at least half)",
                "  costly verdicts       1",
                "  as it stands: no: 1 costly verdict(s)",
                "  not counted: none",
                "",
                "Costly verdicts (row 14 (a): the text rule's verdict, or 8.7's for a stream the text rule sets aside):",
                "  COSTLY disc-a/eng-only stream 1: labelled sdh, an SDH track placed as regular without review",
                "",
                "Labels that cannot be judged:",
                "  none",
                "",
                "Labels in another language (row 14 judges English and no-language tracks only):",
                "  none",
                "",
                "RESULT FAILED: 1 costly verdict(s): disc-a/eng-only stream 1 (sdh_as_regular)",
            ]
        );
        let counts: Vec<(&str, u32)> = h.counts.iter().map(|(kind, n)| (kind.as_str(), *n)).collect();
        assert_eq!(
            counts,
            [
                ("closed-captions", 1),
                ("commentary", 0),
                ("dub-transcript", 0),
                ("forced", 0),
                ("lyrics", 0),
                ("other", 0),
                ("regular", 2),
                ("sdh", 2),
                ("translation", 0),
                ("trivia", 0),
            ]
        );
        assert_eq!(h.costly, [("disc-a/eng-only stream 1".to_string(), Costly::SdhAsRegular)]);
        assert_eq!(
            h.missing,
            [
                "titles with regular and SDH",
                "discs with such titles",
                "feature films with such a title",
                "SDH tracks",
                "regular tracks",
                "forced tracks with events",
                "commentary tracks",
            ]
        );
        assert_eq!((h.placed_without_review, h.labelled, h.exit_code), (3, 5, 1));
    }

    #[test]
    fn with_every_label_judged_and_no_costly_verdict_the_harness_exits_0() {
        let (evidence, text) = trees(&[
            disc_b(vec![("eng-only", 1, "regular", "read", true), ("eng-only", 2, "sdh", "watched", true)], true),
            disc_d(vec![("all-tracks", 2, "closed-captions", "read", true), ("all-tracks", 3, "regular", "read", true)]),
        ]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(line(&h, "  regular "), "  regular               2 of 6");
        assert_eq!(line(&h, "  sdh "), "  sdh                   1 of 6");
        assert_eq!(
            line(&h, "  as it stands: "),
            "  as it stands: not answered: too few titles with regular and SDH, discs with such titles, feature films \
             with such a title, SDH tracks, regular tracks, forced tracks with events, commentary tracks"
        );
        assert_eq!(section(&h, "Costly verdicts ("), ["  none"]);
        assert_eq!(last(&h), "RESULT ok");
        assert_eq!(h.costly, Vec::<(String, Costly)>::new());
        assert_eq!((h.placed_without_review, h.labelled, h.exit_code), (2, 4, 0));
    }

    #[test]
    fn a_row_14_that_is_met_answers_by_the_share_placed_without_review() {
        let (evidence, text) =
            trees(&[disc_d(vec![("all-tracks", 2, "closed-captions", "read", true), ("all-tracks", 3, "regular", "read", true)])]);
        let h = judge(&evidence, Some(&text), &met_row14(), &Settings::default());
        assert_eq!(h.missing, Vec::<String>::new());
        assert_eq!(line(&h, "  as it stands: "), "  as it stands: yes for (a) on the tracks judged (row 14 also asks (b))");
        assert_eq!((h.placed_without_review, h.exit_code), (2, 0));
        let (evidence, text) =
            trees(&[disc_b(vec![("eng-only", 1, "regular", "read", true), ("eng-only", 2, "sdh", "read", true)], true)]);
        let h = judge(&evidence, Some(&text), &met_row14(), &Settings::default());
        assert_eq!(
            line(&h, "  as it stands: "),
            "  as it stands: no: fewer than half of the judged tracks are placed without review"
        );
        assert_eq!(line(&h, "  placed, no review "), "  placed, no review     0 of 2 judged tracks (row 14 (a) asks for at least half)");
        assert_eq!((h.placed_without_review, h.exit_code), (0, 0));
        assert_eq!(last(&h), "RESULT ok");
    }

    #[test]
    fn a_forced_or_commentary_label_on_a_track_placed_as_regular_is_costly() {
        let (evidence, text) = trees(&[disc_a("disc-a", vec![("eng-only", 1, "forced", "read", true)], true)]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(h.costly, [("disc-a/eng-only stream 1".to_string(), Costly::ForcedAsDialogue)]);
        assert_eq!(
            section(&h, "Tracks ("),
            ["  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled forced (read); 8.7 regular; text regular; COSTLY forced_as_dialogue"]
        );
        assert_eq!(line(&h, "  forced "), "  forced                1, with events 1 of 1");
        assert_eq!(
            section(&h, "Costly verdicts ("),
            ["  COSTLY disc-a/eng-only stream 1: labelled forced, a forced track placed as regular or SDH without review"]
        );
        assert_eq!(h.exit_code, 1);
        let (evidence, text) = trees(&[disc_a("disc-a", vec![("eng-only", 1, "commentary", "read", true)], true)]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(h.costly, [("disc-a/eng-only stream 1".to_string(), Costly::UnwantedKept)]);
        assert_eq!(line(&h, "  commentary "), "  commentary            1 of 1");
        assert_eq!(last(&h), "RESULT FAILED: 1 costly verdict(s): disc-a/eng-only stream 1 (unwanted_kept)");
        assert_eq!(h.exit_code, 1);
        let (evidence, text) = trees(&[disc_a("disc-a", vec![("eng-only", 1, "translation", "read", true)], true)]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            ["  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled translation (read); 8.7 regular; text regular; placed"]
        );
        assert_eq!((h.costly.len(), h.exit_code), (0, 0));
    }

    #[test]
    fn a_closed_caption_stream_the_text_rule_sets_aside_is_judged_by_8_7() {
        let (evidence, text) = trees(&[disc_d(vec![("all-tracks", 2, "regular", "read", true)])]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            ["  disc-d/all-tracks stream 2 (scan stream 3, 40 events): labelled regular (read); 8.7 sdh; text set aside; COSTLY regular_as_sdh"]
        );
        assert_eq!(h.costly, [("disc-d/all-tracks stream 2".to_string(), Costly::RegularAsSdh)]);
        assert_eq!(h.exit_code, 1);
    }

    #[test]
    fn labels_without_ocr_text_are_not_judged_and_the_harness_exits_3() {
        let (evidence, _) = trees(&[disc_a("disc-a", vec![("eng-only", 1, "sdh", "read", true)], true)]);
        let h = judge(&evidence, None, &spec_row14(), &Settings::default());
        assert_eq!(
            h.lines[0],
            "subtitle labels: corpus/owner: 1 recordings with subtitle-labels.json, 1 labelled tracks: 0 judged, 1 not judged, 0 in another language"
        );
        assert_eq!(
            section(&h, "Tracks ("),
            ["  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled sdh (read); 8.7 regular; text not read; not judged"]
        );
        assert_eq!(
            section(&h, "Labels that cannot be judged:"),
            ["  disc-a/eng-only stream 1: no OCR text (give --text DIR or LAZYMKV_OCR_TEXT)"]
        );
        assert_eq!(line(&h, "  placed, no review "), "  placed, no review     0 of 0 judged tracks (row 14 (a) asks for at least half)");
        assert_eq!(line(&h, "  as it stands: "), "  as it stands: not answered: 1 labelled track(s) not judged");
        assert_eq!(line(&h, "  not counted: "), "  not counted: disc-a 1");
        assert_eq!(last(&h), "RESULT FAILED: not everything was judged: 1 labelled track(s) (see Labels that cannot be judged)");
        assert_eq!((h.costly.len(), h.placed_without_review, h.labelled, h.exit_code), (0, 0, 1, 3));
        assert_eq!(h.counts["sdh"], 0);
    }

    #[test]
    fn a_title_without_its_ocr_folder_or_with_an_english_stream_left_unread_is_not_judged() {
        let (evidence, text) = trees(&[
            disc_a("disc-a", vec![("eng-only", 1, "sdh", "read", true)], false),
            disc_b(vec![("eng-only", 1, "regular", "read", true), ("eng-only", 2, "sdh", "read", true)], false),
        ]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Labels that cannot be judged:"),
            [
                "  disc-a/eng-only stream 1: no OCR text: text/disc-a/eng-only/subs-ocr (give --text DIR or LAZYMKV_OCR_TEXT)",
                "  disc-b/eng-only stream 1: stream s1 eng: no OCR text (no sub-1.ocr.jsonl in text/disc-b/eng-only/subs-ocr); read it with ocr-subs.ps1",
                "  disc-b/eng-only stream 2: stream s1 eng: no OCR text (no sub-1.ocr.jsonl in text/disc-b/eng-only/subs-ocr); read it with ocr-subs.ps1",
            ]
        );
        assert_eq!(h.exit_code, 3);
    }

    #[test]
    fn without_any_subtitle_label_the_harness_exits_3() {
        let mut evidence = MemTree::new("corpus/owner", "owner");
        evidence.insert_file("disc-a/scan.txt", "CINFO:1,0,\"Blu-ray disc\"\n");
        let h = judge(&evidence, None, &spec_row14(), &Settings::default());
        assert_eq!(
            h.lines[0],
            "subtitle labels: corpus/owner: 0 recordings with subtitle-labels.json, 0 labelled tracks: 0 judged, 0 not judged, 0 in another language"
        );
        assert_eq!(section(&h, "Tracks ("), ["  none"]);
        assert_eq!(last(&h), "RESULT FAILED: no subtitle label to judge (lazymkv-dev labels --subtitles set)");
        assert_eq!((h.labelled, h.exit_code, h.counts.len()), (0, 3, 10));
    }

    #[test]
    fn a_labels_file_that_does_not_validate_stops_the_run_with_exit_2() {
        let (mut evidence, text) = trees(&[disc_a("disc-a", vec![("eng-only", 1, "sdh", "read", true)], true)]);
        let bad = json!({"format": "lazymkv subtitle labels v0", "slug": "disc-x", "tracks": []});
        evidence.insert_file("disc-x/subtitle-labels.json", bad.to_string());
        let problem = load(&evidence, &PathKey::new("disc-x/subtitle-labels.json")).unwrap_err();
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            h.lines,
            [
                "subtitle labels: corpus/owner: 1 labels file(s) do not validate".to_string(),
                format!("  {problem}"),
                String::new(),
                "RESULT FAILED: a subtitle labels file does not validate (lazymkv-dev labels --subtitles check)".to_string(),
            ]
        );
        assert_eq!((h.labelled, h.costly.len(), h.exit_code), (0, 0, 2));
    }

    #[test]
    fn a_rip_or_scan_that_cannot_be_read_or_does_not_verify_leaves_its_labels_unjudged() {
        let mut chapters = disc_a("disc-c", vec![("eng-only", 1, "regular", "read", true)], true);
        chapters.chapters = 3;
        let (mut evidence, text) =
            trees(&[disc_a("disc-a", vec![("absent", 1, "regular", "read", true)], true), chapters]);
        evidence.insert_file("disc-n/subtitle-labels.json", labels_json("disc-n", &[("eng-only", 1, "regular", "read", true)]));
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(section(&h, "Tracks ("), ["  none"]);
        assert_eq!(
            section(&h, "Labels that cannot be judged:"),
            [
                "  disc-a/absent stream 1: absent: no rip-summary.json or extract-summary.json saying ok",
                "  disc-c/eng-only stream 1: eng-only: the rip does not verify against the scan: chapters: expected 2, file 3",
                "  disc-n/eng-only stream 1: [Errno 2] No such file or directory: 'corpus/owner/disc-n/scan.txt'",
            ]
        );
        assert_eq!((h.labelled, h.exit_code), (3, 3));
    }

    #[test]
    fn a_label_on_a_track_that_is_not_a_subtitle_stream_of_the_rip_is_not_judged() {
        let (evidence, text) = trees(&[disc_a(
            "disc-a",
            vec![("eng-only", 0, "regular", "read", true), ("eng-only", 7, "regular", "read", true)],
            true,
        )]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(section(&h, "Tracks ("), ["  none"]);
        assert_eq!(
            section(&h, "Labels that cannot be judged:"),
            [
                "  disc-a/eng-only stream 0: file track 0 is scan stream 0, a video stream, not a subtitle stream",
                "  disc-a/eng-only stream 7: file track 7 is not a track of the rip",
            ]
        );
        assert_eq!(h.exit_code, 3);
    }

    #[test]
    fn a_label_in_another_language_is_listed_apart_and_not_judged() {
        let at = starts(40, 10.0, 3.0);
        let subs = [("eng", at.clone(), false, true), ("spa", at, false, false)];
        let labels = vec![("all-tracks", 1, "regular", "read", true), ("all-tracks", 2, "translation", "read", true)];
        let (evidence, text) = trees(&[bluray("disc-s", None, "all-tracks", ALL_TRACKS, &subs, labels)]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            h.lines[0],
            "subtitle labels: corpus/owner: 1 recordings with subtitle-labels.json, 2 labelled tracks: 1 judged, 0 not judged, 1 in another language"
        );
        assert_eq!(
            section(&h, "Tracks ("),
            ["  disc-s/all-tracks stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular; text regular; placed"]
        );
        assert_eq!(
            section(&h, "Labels in another language ("),
            ["  disc-s/all-tracks stream 2: labelled translation, scan stream 2 is in spa"]
        );
        assert_eq!(line(&h, "  not counted: "), "  not counted: disc-s 1");
        assert_eq!(last(&h), "RESULT ok");
        assert_eq!((h.labelled, h.placed_without_review, h.exit_code), (2, 1, 0));
    }

    #[test]
    fn only_sure_labels_read_or_watched_off_the_fitting_discs_count() {
        let (evidence, text) = trees(&[
            disc_a("disc-a", vec![("eng-only", 1, "regular", "packaging", true)], true),
            disc_a("disc-f", vec![("eng-only", 1, "regular", "read", true)], true),
            disc_a("disc-p", vec![("eng-only", 1, "regular", "read", false)], true),
            disc_a("disc-w", vec![("eng-only", 1, "regular", "watched", true)], true),
        ]);
        let fitted = Row14 { fitting_discs: vec!["disc-f".to_string()], ..spec_row14() };
        let h = judge(&evidence, Some(&text), &fitted, &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled regular (packaging); 8.7 regular; text regular; placed",
                "  disc-f/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular; text regular; placed",
                "  disc-p/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read, not sure); 8.7 regular; text regular; placed",
                "  disc-w/eng-only stream 1 (scan stream 1, 40 events): labelled regular (watched); 8.7 regular; text regular; placed",
            ]
        );
        assert!(line(&h, "Experiment 14 (").contains("the owner sure, never on disc-f; the packaging"), "{:?}", h.lines);
        assert_eq!(line(&h, "  regular "), "  regular               1 of 6");
        assert_eq!(line(&h, "  not counted: "), "  not counted: disc-a 1, disc-f 1, disc-p 1");
        assert_eq!((h.counts["regular"], h.placed_without_review, h.exit_code), (1, 4, 0));
    }

    #[test]
    fn a_label_made_after_seeing_the_verdict_or_from_an_observation_never_counts() {
        let seen = [("eng-only", 1, "regular", "read", true)];
        let (mut evidence, text) = trees(&[
            disc_a("disc-a", seen.to_vec(), true),
            disc_a("disc-w", vec![("eng-only", 1, "regular", "packaging", true)], true),
        ]);
        relabel(&mut evidence, "disc-a", &seen, 0, &[("saw_verdict", json!(true))]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read, saw the verdict); 8.7 regular; text regular; placed",
                "  disc-w/eng-only stream 1 (scan stream 1, 40 events): labelled regular (packaging); 8.7 regular; text regular; placed",
            ]
        );
        assert_eq!(line(&h, "  regular "), "  regular               0 of 6");
        assert_eq!(line(&h, "  not counted: "), "  not counted: disc-a 1, disc-w 1");
        assert_eq!((h.counts["regular"], h.placed_without_review, h.exit_code), (0, 2, 0));
    }

    #[test]
    fn a_uk_style_sdh_label_counts_towards_row_14s_uk_style_track_only_when_the_label_counts() {
        let cued = [("eng-only", 2, "sdh", "read", true)];
        let flagged = [("eng-only", 1, "sdh", "flag", true)];
        let plain = [("eng-only", 1, "regular", "read", true)];
        let seen = [("eng-only", 1, "sdh", "read", true)];
        let (mut evidence, text) = trees(&[
            disc_b(cued.to_vec(), true),
            disc_a("disc-f", flagged.to_vec(), true),
            disc_a("disc-r", plain.to_vec(), true),
            disc_a("disc-v", seen.to_vec(), true),
        ]);
        let uk = ("style", json!("uk"));
        relabel(&mut evidence, "disc-b", &cued, 0, &[uk.clone()]);
        relabel(&mut evidence, "disc-f", &flagged, 0, &[uk.clone()]);
        relabel(&mut evidence, "disc-r", &plain, 0, &[uk.clone()]);
        relabel(&mut evidence, "disc-v", &seen, 0, &[uk, ("saw_verdict", json!(true))]);
        let h = judge(&evidence, Some(&text), &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-b/eng-only stream 2 (scan stream 2, 40 events): labelled sdh (read, style uk); 8.7 sdh (review); text keep (review); reviewed",
                "  disc-f/eng-only stream 1 (scan stream 1, 40 events): labelled sdh (flag, style uk); 8.7 regular; text regular; COSTLY sdh_as_regular",
                "  disc-r/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read, style uk); 8.7 regular; text regular; placed",
                "  disc-v/eng-only stream 1 (scan stream 1, 40 events): labelled sdh (read, saw the verdict, style uk); 8.7 regular; text regular; COSTLY sdh_as_regular",
            ]
        );
        // Only disc-b's label is a UK-style SDH track that counts: disc-f's is an observation (a flag, D23),
        // disc-r's is not an sdh label, and disc-v's was given after the verdict was seen.
        assert_eq!(
            line(&h, "  UK-style SDH "),
            "  UK-style SDH          1 of 1 counted SDH tracks labelled style uk (only if the owner has a UK disc)"
        );
        assert_eq!(line(&h, "  sdh "), "  sdh                   1 of 6");
        assert_eq!(line(&h, "  regular "), "  regular               1 of 6");
        assert_eq!(line(&h, "  not counted: "), "  not counted: disc-f 1, disc-v 1");
        // Row 14 asks for the UK-style track only if the owner has a UK disc, so it is never a missing kind.
        assert!(h.missing.iter().all(|name| !name.contains("UK")), "{:?}", h.missing);
        assert_eq!((h.counts["sdh"], h.exit_code), (1, 1));
    }

    #[test]
    fn sub_clear_max_bytes_decides_which_packets_are_events() {
        let (evidence, _) = trees(&[disc_a("disc-a", vec![("eng-only", 1, "sdh", "read", true)], true)]);
        let h = judge(&evidence, None, &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            ["  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled sdh (read); 8.7 regular; text not read; not judged"]
        );
        // At 29 bytes the 30-byte clear packets are events too (spec 8.7 "Events").
        let low = Settings { sub_clear_max_bytes: 29, ..Settings::default() };
        let h = judge(&evidence, None, &spec_row14(), &low);
        assert_eq!(
            h.lines[1],
            "rule: 8.7 with sub_clear_max_bytes 29, subs_tolerance_ms 300, subs_same_min 0.7, subs_diff_max 0.3; the text rule (EXP-14-design.md section 3) with the same four and its other thresholds as in the code, all provisional"
        );
        assert_eq!(
            section(&h, "Tracks ("),
            ["  disc-a/eng-only stream 1 (scan stream 1, 80 events): labelled sdh (read); 8.7 regular; text not read; not judged"]
        );
    }

    #[test]
    fn subs_tolerance_ms_decides_whether_two_tracks_align() {
        let mut later = starts(40, 10.5, 3.0);
        later.extend(starts(4, 300.0, 3.0));
        let subs = [("eng", starts(40, 10.0, 3.0), false, false), ("eng", later, false, false)];
        let labels = vec![("eng-only", 1, "regular", "read", true), ("eng-only", 2, "sdh", "read", true)];
        let (evidence, _) = trees(&[bluray("disc-t", None, "eng-only", ENG_ONLY, &subs, labels)]);
        // 0.5 s apart: no start within 300 ms, so the two full tracks are in different groups and none is picked:
        // the first group stands in and the other full track is kept as regular under the review (Task 10, D15,
        // D20).
        let h = judge(&evidence, None, &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-t/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular (review); text not read; not judged",
                "  disc-t/eng-only stream 2 (scan stream 2, 44 events): labelled sdh (read); 8.7 regular (review); text not read; not judged",
            ]
        );
        // Within 600 ms every start aligns: one group, the track with more events is SDH.
        let wide = Settings { subs_tolerance_ms: 600, ..Settings::default() };
        let h = judge(&evidence, None, &spec_row14(), &wide);
        assert!(h.lines[1].starts_with("rule: 8.7 with sub_clear_max_bytes 64, subs_tolerance_ms 600,"), "{}", h.lines[1]);
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-t/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular; text not read; not judged",
                "  disc-t/eng-only stream 2 (scan stream 2, 44 events): labelled sdh (read); 8.7 sdh; text not read; not judged",
            ]
        );
    }

    #[test]
    fn subs_same_min_decides_when_a_share_joins_one_group() {
        let mut half = starts(20, 10.0, 3.0);
        half.extend(starts(20, 71.0, 3.0));
        half.extend(starts(4, 300.0, 3.0));
        let subs = [("eng", starts(40, 10.0, 3.0), false, false), ("eng", half, false, false)];
        let labels = vec![("eng-only", 1, "regular", "read", true), ("eng-only", 2, "sdh", "read", true)];
        let (evidence, _) = trees(&[bluray("disc-m", None, "eng-only", ENG_ONLY, &subs, labels)]);
        // A share of 0.5 lies between 0.30 and 0.70: subtitle_ambiguous, two groups, none picked; the first stands
        // in and the other full track is kept as regular under the review (Task 10, D15, D20).
        let h = judge(&evidence, None, &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-m/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular (review); text not read; not judged",
                "  disc-m/eng-only stream 2 (scan stream 2, 44 events): labelled sdh (read); 8.7 regular (review); text not read; not judged",
            ]
        );
        let lower = Settings { subs_same_min: 0.5, ..Settings::default() };
        let h = judge(&evidence, None, &spec_row14(), &lower);
        assert!(h.lines[1].contains("subs_same_min 0.5, subs_diff_max 0.3;"), "{}", h.lines[1]);
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-m/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular; text not read; not judged",
                "  disc-m/eng-only stream 2 (scan stream 2, 44 events): labelled sdh (read); 8.7 sdh; text not read; not judged",
            ]
        );
    }

    #[test]
    fn subs_diff_max_decides_when_a_share_parts_two_groups() {
        let mut half = starts(20, 10.0, 3.0);
        half.extend(starts(20, 71.0, 3.0));
        let mut more = starts(40, 10.0, 3.0);
        more.extend(starts(4, 300.0, 3.0));
        let subs = [("eng", starts(40, 10.0, 3.0), false, false), ("eng", half, false, false), ("eng", more, false, false)];
        let labels = vec![
            ("eng-only", 1, "regular", "read", true),
            ("eng-only", 2, "commentary", "read", true),
            ("eng-only", 3, "sdh", "read", true),
        ];
        let (evidence, _) = trees(&[bluray("disc-3", None, "eng-only", ENG_ONLY, &subs, labels)]);
        // Streams 1 and 3 align fully and form the picked (largest) group; stream 2 shares half its starts with
        // each: between the bands, a review, and a track Between with a track of the picked group is kept as
        // regular under it, never dropped unconfirmed (Task 10, D20).
        let h = judge(&evidence, None, &spec_row14(), &Settings::default());
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-3/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular (review); text not read; not judged",
                "  disc-3/eng-only stream 2 (scan stream 2, 40 events): labelled commentary (read); 8.7 regular (review); text not read; not judged",
                "  disc-3/eng-only stream 3 (scan stream 3, 44 events): labelled sdh (read); 8.7 sdh (review); text not read; not judged",
            ]
        );
        // At 0.5 the half share is Different from both: no review; the larger group is the dialogue group and the
        // track outside it is dropped (OutsideDialogueGroup).
        let wider = Settings { subs_diff_max: 0.5, ..Settings::default() };
        let h = judge(&evidence, None, &spec_row14(), &wider);
        assert!(h.lines[1].contains("subs_same_min 0.7, subs_diff_max 0.5;"), "{}", h.lines[1]);
        assert_eq!(
            section(&h, "Tracks ("),
            [
                "  disc-3/eng-only stream 1 (scan stream 1, 40 events): labelled regular (read); 8.7 regular; text not read; not judged",
                "  disc-3/eng-only stream 2 (scan stream 2, 40 events): labelled commentary (read); 8.7 dropped; text not read; not judged",
                "  disc-3/eng-only stream 3 (scan stream 3, 44 events): labelled sdh (read); 8.7 sdh; text not read; not judged",
            ]
        );
    }
}
