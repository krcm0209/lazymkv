//! The pinned public TheDiscDB scan logs of `corpus/thediscdb` against what `core` computes from them: the fingerprint
//! of spec 5 (`fingerprint::content_hash` of each log's HSH lines equals its parsed form's `ContentHash`), the typed disc
//! model of spec 9.4 and 9.5 (`disc::disc`, with the values read from each log), and the title matching of spec 5 and
//! 6.3 step 8 (`disc::same_scan_title`) on titles that share a segment map. Needs no evidence clone.

use lazymkv_core::codec::Family;
use lazymkv_core::disc::{disc, same_scan_title, segment_maps_agree, Disc, DiscKind, Flags, StreamKind, TitleSource};
use lazymkv_core::fingerprint::{content_hash, hashed_files};
use lazymkv_core::recording::{decode_text, Encoding, Errors};
use lazymkv_core::robot::{parse, Scan};
use std::fs;
use std::path::{Path, PathBuf};

fn scan_logs(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            scan_logs(&path, found);
        } else if name.starts_with("disc") && name.ends_with(".txt") && !name.ends_with("-summary.txt") {
            found.push(path);
        }
    }
}

#[test]
fn content_hash_matches_every_pinned_thediscdb_log() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb");
    let mut logs = Vec::new();
    scan_logs(&root, &mut logs);
    assert_eq!(logs.len(), 9, "expected the nine pinned logs under {}", root.display());

    for log_path in logs {
        let log = String::from_utf8_lossy(&fs::read(&log_path).unwrap()).into_owned();
        let parsed: serde_json::Value =
            serde_json::from_slice(&fs::read(log_path.with_extension("json")).unwrap()).unwrap();
        let expected = parsed["ContentHash"].as_str().unwrap();
        assert_eq!(
            content_hash(&hashed_files(&log)).as_deref(),
            Some(expected),
            "{}",
            log_path.display()
        );
    }
}

const JOHN_WICK_UHD: &str = "movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt";
const JOHN_WICK_BLU_RAY: &str = "movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc02.txt";
const KNOWING: &str = "movie/Knowing (2009)/2010-dvd/disc01.txt";
const SMILE: &str = "movie/Smile (2022)/2022-4k/disc01.txt";
const EXPENDABLES: &str = "movie/The Expendables 3 (2014)/the-expendables-3-film-collection/disc01.txt";
const CHERNOBYL_1: &str = "series/Chernobyl (2019)/2020-4k-US/disc01.txt";
const CHERNOBYL_2: &str = "series/Chernobyl (2019)/2020-4k-US/disc02.txt";
const FIREFLY_1: &str = "series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt";
const FIREFLY_2: &str = "series/Firefly (2002)/2003-dvd-the-complete-series/disc02.txt";

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb")
}

/// The pinned log at `log` (a path below corpus/thediscdb), parsed.
fn pinned_scan(log: &str) -> Scan {
    let path = corpus().join(log);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {}", path.display(), error));
    let text = decode_text(&bytes, Encoding::Utf8, Errors::Replace).unwrap();
    parse(&text)
}

/// The pinned log at `log`, typed.
fn pinned(log: &str) -> Disc {
    let scan = pinned_scan(log);
    disc(&scan).unwrap_or_else(|error| panic!("{log}: {error}"))
}

/// What the disc model gives for one pinned log. `streams` is (video, audio, subtitles); the flag columns count the
/// streams of every title for which the query holds; `unknown_bits` counts streams with a bit outside Flags::KNOWN.
struct Pinned {
    log: &'static str,
    kind: DiscKind,
    name: &'static str,
    volume_label: &'static str,
    comment: Option<&'static str>,
    metadata_language: Option<&'static str>,
    banner: &'static str,
    version: &'static str,
    java: bool,
    is_dvd: bool,
    titles: usize,
    seconds: u64,
    bytes: u64,
    chapters: u64,
    streams: (usize, usize, usize),
    commentary: usize,
    derived_forced_only: usize,
    forced_authored: usize,
    derived: usize,
    unknown_bits: usize,
}

const PINNED: [Pinned; 9] = [
    Pinned {
        log: JOHN_WICK_UHD,
        kind: DiscKind::BluRay,
        name: "JOHN_WICK_UHD",
        volume_label: "JOHN_WICK_UHD",
        comment: None,
        metadata_language: Some("eng"),
        banner: "MakeMKV v1.18.4 win(x64-release)",
        version: "1.18.4",
        java: false,
        is_dvd: false,
        titles: 10,
        seconds: 15287,
        bytes: 147_974_916_096,
        chapters: 18,
        streams: (10, 22, 18),
        commentary: 0,
        derived_forced_only: 9,
        forced_authored: 0,
        derived: 12,
        unknown_bits: 3,
    },
    Pinned {
        log: JOHN_WICK_BLU_RAY,
        kind: DiscKind::BluRay,
        name: "JOHN_WICK",
        volume_label: "JOHN_WICK",
        comment: None,
        metadata_language: Some("eng"),
        banner: "MakeMKV v1.18.4 win(x64-release)",
        version: "1.18.4",
        java: false,
        is_dvd: false,
        titles: 9,
        seconds: 9214,
        bytes: 37_580_132_352,
        chapters: 18,
        streams: (9, 14, 8),
        commentary: 0,
        derived_forced_only: 4,
        forced_authored: 0,
        derived: 7,
        unknown_bits: 0,
    },
    Pinned {
        log: KNOWING,
        kind: DiscKind::Dvd,
        name: "KNOWING",
        volume_label: "KNOWING",
        comment: None,
        metadata_language: None,
        banner: "MakeMKV v1.18.3 win(x64-release)",
        version: "1.18.3",
        java: false,
        is_dvd: true,
        titles: 7,
        seconds: 9454,
        bytes: 8_292_057_088,
        chapters: 29,
        streams: (7, 8, 8),
        commentary: 3,
        derived_forced_only: 0,
        forced_authored: 0,
        derived: 0,
        unknown_bits: 0,
    },
    Pinned {
        log: SMILE,
        kind: DiscKind::BluRay,
        name: "Smile",
        volume_label: "SMILE",
        comment: Some("UHD-141056_Smile_WW"),
        metadata_language: Some("eng"),
        banner: "MakeMKV v1.18.2 win(x64-release)",
        version: "1.18.2",
        java: true,
        is_dvd: false,
        titles: 236,
        seconds: 20514,
        bytes: 174_908_608_512,
        chapters: 203,
        streams: (236, 38, 234),
        commentary: 0,
        derived_forced_only: 117,
        forced_authored: 0,
        derived: 121,
        unknown_bits: 227,
    },
    Pinned {
        log: EXPENDABLES,
        kind: DiscKind::BluRay,
        name: "Expendables 3",
        volume_label: "EXPENDABLES_3",
        comment: Some("Expendables3_Retail"),
        metadata_language: Some("eng"),
        banner: "MakeMKV v1.18.1 win(x86-release)",
        version: "1.18.1",
        java: true,
        is_dvd: false,
        titles: 64,
        seconds: 77981,
        bytes: 276_530_436_096,
        chapters: 183,
        streams: (64, 161, 212),
        commentary: 0,
        derived_forced_only: 106,
        forced_authored: 0,
        derived: 142,
        unknown_bits: 0,
    },
    Pinned {
        log: CHERNOBYL_1,
        kind: DiscKind::BluRay,
        name: "Chernobyl Disc 1",
        volume_label: "CHERNOBYL_D1",
        comment: None,
        metadata_language: Some("eng"),
        banner: "MakeMKV v1.18.3 win(x64-release)",
        version: "1.18.3",
        java: false,
        is_dvd: false,
        titles: 33,
        seconds: 24875,
        bytes: 187_463_325_696,
        chapters: 79,
        streams: (33, 41, 228),
        commentary: 0,
        derived_forced_only: 114,
        forced_authored: 0,
        derived: 121,
        unknown_bits: 23,
    },
    Pinned {
        log: CHERNOBYL_2,
        kind: DiscKind::BluRay,
        name: "Chernobyl Disc 2",
        volume_label: "CHERNOBYL_D2",
        comment: None,
        metadata_language: Some("eng"),
        banner: "MakeMKV v1.18.3 win(x64-release)",
        version: "1.18.3",
        java: false,
        is_dvd: false,
        titles: 31,
        seconds: 18509,
        bytes: 125_116_256_256,
        chapters: 68,
        streams: (31, 28, 152),
        commentary: 0,
        derived_forced_only: 76,
        forced_authored: 0,
        derived: 81,
        unknown_bits: 24,
    },
    Pinned {
        log: FIREFLY_1,
        kind: DiscKind::Dvd,
        name: "FIREFLY_D1",
        volume_label: "FIREFLY_D1",
        comment: None,
        metadata_language: None,
        banner: "MakeMKV v1.18.4 win(x64-release)",
        version: "1.18.4",
        java: false,
        is_dvd: true,
        titles: 5,
        seconds: 10385,
        bytes: 7_870_365_696,
        chapters: 51,
        streams: (5, 12, 15),
        commentary: 2,
        derived_forced_only: 0,
        forced_authored: 6,
        derived: 0,
        unknown_bits: 0,
    },
    Pinned {
        log: FIREFLY_2,
        kind: DiscKind::Dvd,
        name: "FIREFLY_D2",
        volume_label: "FIREFLY_D2",
        comment: None,
        metadata_language: None,
        banner: "MakeMKV v1.18.4 win(x64-release)",
        version: "1.18.4",
        java: false,
        is_dvd: true,
        titles: 6,
        seconds: 10465,
        bytes: 7_891_234_816,
        chapters: 56,
        streams: (6, 14, 20),
        commentary: 1,
        derived_forced_only: 0,
        forced_authored: 8,
        derived: 0,
        unknown_bits: 0,
    },
];

#[test]
fn the_disc_model_types_every_pinned_log() {
    for want in &PINNED {
        let log = want.log;
        let scan = pinned_scan(log);
        let got = disc(&scan).unwrap_or_else(|error| panic!("{log}: {error}"));
        assert_eq!(got.kind, want.kind, "{log}");
        assert_eq!(got.name.as_deref(), Some(want.name), "{log}");
        assert_eq!(got.tree_name.as_deref(), Some(want.name), "{log}: CINFO 30 equals CINFO 2 on every pinned log");
        assert_eq!(got.volume_label.as_deref(), Some(want.volume_label), "{log}");
        assert_eq!(got.comment.as_deref(), want.comment, "{log}");
        assert_eq!(got.metadata_language.as_deref(), want.metadata_language, "{log}");
        assert_eq!(got.makemkv_banner.as_deref(), Some(want.banner), "{log}");
        assert_eq!(got.makemkv_version.as_deref(), Some(want.version), "{log}");
        assert_eq!(got.java_in_use, want.java, "{log}");
        assert_eq!(got.is_dvd, want.is_dvd, "{log}");
        assert_eq!(got.scan_read_errors, 0, "{log}");
        assert_eq!(got.titles.len(), want.titles, "{log}");
        let seconds: u64 = got
            .titles
            .values()
            .map(|title| u64::from(title.seconds.unwrap_or_else(|| panic!("{log} title {}: no duration", title.number))))
            .sum();
        assert_eq!(seconds, want.seconds, "{log}");
        let bytes: u64 = got.titles.values().filter_map(|title| title.size).sum();
        assert_eq!(bytes, want.bytes, "{log}");
        let chapters: u64 = got.titles.values().map(|title| u64::from(title.chapters)).sum();
        assert_eq!(chapters, want.chapters, "{log}");
        for title in got.titles.values() {
            let in_scan = scan.streams.keys().filter(|(number, _)| *number == title.number).count();
            assert_eq!(title.streams.len(), in_scan, "{log} title {}: every stream has a known type", title.number);
            let by_dvd_title = matches!(title.source(), TitleSource::DvdTitle { .. });
            assert_eq!(by_dvd_title, want.is_dvd, "{log} title {}", title.number);
        }
        let all: Vec<&lazymkv_core::disc::Stream> = got.titles.values().flat_map(|title| title.streams.iter()).collect();
        let count = |kind: StreamKind| all.iter().filter(|stream| stream.kind == kind).count();
        assert_eq!(
            (count(StreamKind::Video), count(StreamKind::Audio), count(StreamKind::Subtitles)),
            want.streams,
            "{log}"
        );
        let flagged = |query: fn(Flags) -> bool| all.iter().filter(|stream| query(stream.flags)).count();
        assert_eq!(flagged(Flags::commentary), want.commentary, "{log}: commentary");
        assert_eq!(flagged(Flags::derived_forced_only), want.derived_forced_only, "{log}: derived forced-only");
        assert_eq!(flagged(Flags::forced_authored), want.forced_authored, "{log}: forced");
        assert_eq!(flagged(|flags: Flags| flags.has(Flags::DERIVED)), want.derived, "{log}: derived");
        assert_eq!(flagged(|flags: Flags| flags.0 & !Flags::KNOWN != 0), want.unknown_bits, "{log}: unknown bits");
    }
}

#[test]
fn pinned_titles_carry_their_sources_comments_durations_and_segment_maps() {
    let wick = pinned(JOHN_WICK_UHD);
    let feature = wick.title(0).unwrap();
    assert_eq!(feature.source(), TitleSource::File { name: "00001.mpls".to_string() });
    assert_eq!((feature.seconds, feature.chapters, feature.size), (Some(6073), 16, Some(67_757_930_496)));
    assert_eq!((feature.segment_count, feature.segment_map.as_str(), feature.truncated), (Some(1), "1", false));
    assert_eq!(feature.segments, [1]);
    assert_eq!(feature.output_name.as_deref(), Some("title_t00.mkv"));
    assert_eq!((feature.name.as_deref(), feature.comment.as_deref()), (None, None));
    assert_eq!(feature.video_height(), Some(2160));
    let clip = wick.title(2).unwrap();
    assert_eq!(clip.source(), TitleSource::File { name: "00007.m2ts".to_string() });
    assert_eq!((clip.seconds, clip.chapters), (Some(360), 0));
    assert_eq!(pinned(JOHN_WICK_BLU_RAY).title(0).unwrap().video_height(), Some(1080));

    let knowing = pinned(KNOWING);
    let main = knowing.title(0).unwrap();
    assert_eq!(main.source(), TitleSource::DvdTitle { number: 1 });
    assert_eq!((main.source_file.as_deref(), main.dvd_title.as_deref()), (None, Some("01")));
    assert_eq!(main.comment.as_deref(), Some("B1"));
    assert_eq!((main.seconds, main.chapters, main.size), (Some(7274), 18, Some(7_098_400_768)));
    assert_eq!(main.segment_map, "1-19");
    assert_eq!(main.segments, (1..=19).collect::<Vec<i64>>());
    assert_eq!(main.output_name.as_deref(), Some("title.mkv"));
    assert_eq!(main.video_height(), Some(480));

    let smile = pinned(SMILE);
    let main_feature = smile.title(14).unwrap();
    assert_eq!(main_feature.source(), TitleSource::File { name: "00040.mpls".to_string() });
    assert_eq!(main_feature.comment.as_deref(), Some("FPL_MainFeature"));
    assert_eq!(main_feature.name.as_deref(), Some("Smile"));
    assert_eq!((main_feature.seconds, main_feature.segment_map.as_str()), (Some(6937), "294"));

    let expendables = pinned(EXPENDABLES);
    let unrated = expendables.title(41).unwrap();
    assert_eq!(unrated.source(), TitleSource::File { name: "00756.mpls".to_string() });
    assert_eq!(unrated.comment.as_deref(), Some("FPL_MainFeature_UR"));
    assert_eq!((unrated.seconds, unrated.segment_count, unrated.segments.len()), (Some(7891), Some(17), 17));
    assert_eq!(unrated.segments[0], 280);
    let rated = expendables.title(54).unwrap();
    assert_eq!(rated.source(), TitleSource::File { name: "00820.mpls".to_string() });
    assert_eq!((rated.comment.as_deref(), rated.seconds), (Some("FPL_MainFeature_R"), Some(7587)));

    let firefly = pinned(FIREFLY_2);
    let last = firefly.title(5).unwrap();
    assert_eq!(last.source(), TitleSource::DvdTitle { number: 11 });
    assert_eq!((last.dvd_title.as_deref(), last.comment.as_deref(), last.seconds), (Some("11"), Some("F1"), Some(2628)));
    assert_eq!(last.segments, (1..=15).collect::<Vec<i64>>());
}

#[test]
fn pinned_streams_carry_their_codecs_bitrates_layouts_and_flags() {
    let wick = pinned(JOHN_WICK_UHD);
    let feature = wick.title(0).unwrap();
    assert_eq!(feature.streams.iter().map(|stream| stream.number).collect::<Vec<u32>>(), (0..12).collect::<Vec<u32>>());
    assert_eq!(
        (feature.streams_of(StreamKind::Audio).len(), feature.streams_of(StreamKind::Subtitles).len()),
        (5, 6)
    );
    let video = &feature.streams[0];
    assert_eq!(
        (video.kind, video.language.clone(), video.codec.family, video.video_size, video.flags),
        (StreamKind::Video, None, Family::Hevc, Some((3840, 2160)), Flags(65536))
    );
    assert_eq!(video.codec_long, "MpegH HEVC Main10@L5.1 (dvhe.07.06 BL+FEL+RPU)");
    let atmos = &feature.streams[1];
    assert_eq!(
        (atmos.kind, atmos.name.as_deref(), atmos.language.as_deref(), atmos.codec.family),
        (StreamKind::Audio, Some("Surround 7.1"), Some("eng"), Family::TrueHd)
    );
    assert_eq!(atmos.codec_long, "TrueHD Atmos");
    assert_eq!((atmos.bitrate_text.as_deref(), atmos.bitrate_kbps), (Some("128 Kb/s"), Some(128)));
    assert_eq!((atmos.channels, atmos.sample_rate, atmos.sample_size), (Some(8), Some(48000), Some(24)));
    assert_eq!((atmos.flags, atmos.layout_name.as_deref()), (Flags(1024), Some("7.1")));
    assert_eq!((atmos.output_codec.as_deref(), atmos.conversion.as_deref()), (None, Some("( Lossless conversion )")));
    let core = &feature.streams[2];
    assert_eq!((core.codec.family, core.bitrate_kbps, core.flags), (Family::Ac3, Some(640), Flags(2304)));
    assert!(core.flags.has(Flags::DERIVED) && core.flags.has(Flags::CORE_AUDIO));
    let forced = &feature.streams[9];
    assert_eq!(
        (forced.kind, forced.language.as_deref(), forced.codec.family, forced.flags),
        (StreamKind::Subtitles, Some("eng"), Family::Pgs, Flags(6144))
    );
    assert!(forced.flags.derived_forced_only());
    assert_eq!(forced.tree_info.as_deref(), Some("PGS English  (forced only)"));

    let knowing = pinned(KNOWING);
    let main = knowing.title(0).unwrap();
    let commentary: Vec<(u32, StreamKind)> =
        main.streams.iter().filter(|stream| stream.flags.commentary()).map(|stream| (stream.number, stream.kind)).collect();
    assert_eq!(commentary, [(3, StreamKind::Audio), (6, StreamKind::Subtitles), (7, StreamKind::Subtitles)]);
    assert_eq!((main.streams[0].codec.family, main.streams[0].bitrate_kbps), (Family::Mpeg2, Some(9800)));
    assert_eq!(main.streams[3].bitrate_kbps, Some(192));
    let vobsub = &main.streams[4];
    assert_eq!((vobsub.codec.family, vobsub.codec_short.as_str(), vobsub.codec_long.as_str()), (Family::VobSub, "", "Dvd Subtitles"));
    assert_eq!((vobsub.language.as_deref(), vobsub.tree_info.as_deref()), (Some("eng"), Some(" English")));

    let firefly = pinned(FIREFLY_1);
    let pilot = firefly.title(0).unwrap();
    let forced: Vec<u32> =
        pilot.streams.iter().filter(|stream| stream.flags.forced_authored()).map(|stream| stream.number).collect();
    assert_eq!(forced, [7, 8]);
    let captions = &pilot.streams[9];
    assert_eq!(
        (captions.kind, captions.language.as_deref(), captions.codec.family),
        (StreamKind::Subtitles, Some("eng"), Family::ClosedCaptions)
    );
    assert_eq!(
        (captions.output_codec.as_deref(), captions.conversion.as_deref(), captions.bitrate_kbps),
        (Some("Text"), Some("( Lossy conversion )"), Some(9800))
    );
}

#[test]
fn john_wicks_feature_playlist_and_its_m2ts_clip_are_different_titles() {
    let wick = pinned(JOHN_WICK_UHD);
    let playlist = wick.title(0).unwrap();
    let clip = wick.title(8).unwrap();
    assert_eq!((playlist.segment_map.as_str(), clip.segment_map.as_str()), ("1", "1"));
    assert_eq!((playlist.seconds, clip.seconds), (Some(6073), Some(6073)));
    assert!(!same_scan_title(playlist, clip));
    for title in wick.titles.values() {
        assert!(same_scan_title(title, title), "title {}", title.number);
    }
}

#[test]
fn firefly_disc_2s_first_and_third_episodes_differ_by_attribute_24_alone() {
    let firefly = pinned(FIREFLY_2);
    let first = firefly.title(0).unwrap();
    let third = firefly.title(2).unwrap();
    assert!(segment_maps_agree(&first.segment_map, &third.segment_map));
    assert_eq!((first.seconds, third.seconds), (Some(2631), Some(2629)));
    assert_eq!(
        (first.source(), third.source()),
        (TitleSource::DvdTitle { number: 1 }, TitleSource::DvdTitle { number: 3 })
    );
    assert!(!same_scan_title(first, third));
    assert!(same_scan_title(first, first));
}
