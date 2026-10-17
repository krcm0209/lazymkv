//! The typed disc model of spec 9.4 and 9.5 (`lazymkv_core::disc`, phase 1 Task 2) over every recorded `scan.txt` of
//! the evidence clone, the title matching of spec 6.3 step 8 on titles that share a segment map or a duration, and the
//! disc name of spec 6.1 step 2 from each recording's `nav/BDMV/META/DL/bdmt_*.xml`. Each recording has a test that
//! writes out only the values the public spec, docs and fixtures publish and compares every other value with the raw
//! attribute of the same scan (ruling RP21); a recording added later is listed by
//! recordings_without_a_test_are_listed_as_not_covered until it gets its test here. Without the clone, or without one
//! recording, a test prints "skipped: <reason>" and passes. The scans are never printed: their DRV lines can carry a
//! drive serial. Input: LAZYMKV_EVIDENCE, else corpus/owner under the repository root.

use std::path::{Path, PathBuf};

use lazymkv_core::audio::titles::segments;
use lazymkv_core::codec::{bitrate_kbps, Family};
use lazymkv_core::disc::{
    bdmt_candidates, bdmt_name, disc, same_scan_title, Disc, DiscKind, Flags, StreamKind, TitleSource,
};
use lazymkv_core::recording::{decode_text, Encoding, Errors};
use lazymkv_core::robot::{duration_seconds, parse, Scan};
use lazymkv_dev::evidence::skip;

/// The recordings with a test here.
const RECORDINGS: [&str; 4] =
    ["pans-labyrinth-2006-uhd", "short-circuit-1986-dvd", "short-circuit-2-1988-dvd", "the-office-2005-s3d3-bluray"];

/// The evidence clone: LAZYMKV_EVIDENCE, else corpus/owner under the repository root.
fn evidence() -> PathBuf {
    match std::env::var("LAZYMKV_EVIDENCE") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/owner"),
    }
}

/// The slugs of the clone's recordings that have a scan.txt (never the golden folder), sorted; None, with the skip line
/// written, when the clone is absent.
fn recorded_slugs() -> Option<Vec<String>> {
    let root = evidence();
    if !root.is_dir() {
        skip(&format!("no evidence clone at {}", root.display()));
        return None;
    }
    let mut slugs = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let dir = entry.unwrap().path();
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        if name != "golden" && dir.join("scan.txt").is_file() {
            slugs.push(name);
        }
    }
    slugs.sort();
    Some(slugs)
}

/// The parsed scan.txt of `slug`; None, with the skip line written, when the clone or the recording is absent.
fn recorded_scan(slug: &str) -> Option<Scan> {
    let path = evidence().join(slug).join("scan.txt");
    if !path.is_file() {
        skip(&format!("no {} (the evidence clone or this recording is absent)", path.display()));
        return None;
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {}", path.display(), error));
    let text = decode_text(&bytes, Encoding::Utf8Sig, Errors::Replace)
        .unwrap_or_else(|error| panic!("{}: {}", path.display(), error));
    Some(parse(&text))
}

/// Disc attribute (CINFO) `id` of `scan` as the scan wrote it; None when absent. Ruling RP21: a value the public docs
/// do not publish is compared with the raw attribute of the same scan, never written in this file.
fn disc_attr(scan: &Scan, id: u32) -> Option<&str> {
    scan.disc.get(&id).map(String::as_str)
}

/// Title attribute `id` of `scan` as the scan wrote it; None when absent.
fn title_attr(scan: &Scan, title: u32, id: u32) -> Option<&str> {
    scan.titles.get(&title).and_then(|attrs| attrs.get(&id)).map(String::as_str)
}

/// Title attribute `id` read as a number; None when absent or not a number.
fn title_number(scan: &Scan, title: u32, id: u32) -> Option<u64> {
    title_attr(scan, title, id).and_then(|text| text.parse().ok())
}

/// Title attribute 9 through `robot::duration_seconds`; None when absent or unparsable.
fn title_seconds(scan: &Scan, title: u32) -> Option<u32> {
    title_attr(scan, title, 9).and_then(duration_seconds)
}

/// `codec::bitrate_kbps` of stream attribute 13; None when absent or unreadable.
fn stream_kbps(scan: &Scan, title: u32, stream: u32) -> Option<u32> {
    scan.streams.get(&(title, stream)).and_then(|attrs| attrs.get(&13)).and_then(|text| bitrate_kbps(text))
}

/// The typed disc of `slug`'s scan; a refusal fails the test with its sentence.
fn typed_disc(slug: &str, scan: &Scan) -> Disc {
    disc(scan).unwrap_or_else(|error| panic!("{slug}: {error}"))
}

/// The disc name of `slug`'s nav files: the first of `bdmt_candidates` in nav/BDMV/META/DL, read through `bdmt_name`;
/// None when that folder or a bdmt file is absent (a DVD, or a Blu-ray recorded without disc metadata).
fn recorded_bdmt_name(slug: &str) -> Option<String> {
    let folder = evidence().join(slug).join("nav/BDMV/META/DL");
    let entries = std::fs::read_dir(&folder).ok()?;
    let names: Vec<String> = entries.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    let first = bdmt_candidates(&names).into_iter().next()?;
    let bytes = std::fs::read(folder.join(&first)).unwrap_or_else(|error| panic!("{first}: {error}"));
    let text = String::from_utf8_lossy(&bytes).into_owned();
    bdmt_name(&text)
}

#[test]
fn every_recorded_scan_gives_a_typed_disc() {
    let Some(slugs) = recorded_slugs() else { return };
    if slugs.is_empty() {
        skip(&format!("no recording with a scan.txt under {}", evidence().display()));
        return;
    }
    for slug in &slugs {
        let scan = recorded_scan(slug).unwrap();
        let typed = typed_disc(slug, &scan);
        assert_eq!(typed.titles.len(), scan.titles.len(), "{slug}: titles");
        for title in typed.titles.values() {
            let in_scan = scan.streams.keys().filter(|(number, _)| *number == title.number).count();
            assert_eq!(title.streams.len(), in_scan, "{slug} title {}: every stream has a known type", title.number);
            assert!(title.seconds.is_some(), "{slug} title {}: no duration", title.number);
            let by_dvd_title = matches!(title.source(), TitleSource::DvdTitle { .. });
            assert_eq!(by_dvd_title, typed.is_dvd, "{slug} title {}: known by attribute 24 on a DVD only", title.number);
        }
    }
}

#[test]
fn pans_labyrinth_2006_uhd() {
    let slug = "pans-labyrinth-2006-uhd";
    let Some(scan) = recorded_scan(slug) else { return };
    let pans = typed_disc(slug, &scan);
    assert_eq!(pans.kind, DiscKind::BluRay);
    assert_eq!(
        (pans.name.as_deref(), pans.tree_name.as_deref(), pans.volume_label.as_deref()),
        (Some("Pan's Labyrinth"), Some("Pan's Labyrinth"), Some("PANS_LABYRINTH"))
    );
    assert_eq!((pans.comment.as_deref(), pans.metadata_language.as_deref()), (None, Some("eng")));
    assert_eq!(pans.makemkv_banner.as_deref(), Some("MakeMKV v2.0.0 win(x64-release)"));
    assert_eq!((pans.makemkv_version.as_deref(), pans.java_in_use), (Some("2.0.0"), false));
    assert_eq!((pans.java_in_use, pans.is_dvd, pans.scan_read_errors), (false, false, 0));
    assert_eq!(pans.titles.keys().copied().collect::<Vec<u32>>(), [0, 1]);

    let feature = pans.title(0).unwrap();
    assert_eq!(feature.source(), TitleSource::File { name: "00800.mpls".to_string() });
    assert_eq!((feature.seconds, feature.chapters, feature.size), (Some(7150), 22, Some(55_613_952_000)));
    assert_eq!((feature.segment_map.as_str(), feature.segments.clone()), ("6", vec![6]));
    assert_eq!(feature.video_height(), Some(2160));
    assert_eq!(feature.streams.iter().map(|stream| stream.number).collect::<Vec<u32>>(), (0..8).collect::<Vec<u32>>());
    let lossless = &feature.streams[1];
    assert_eq!(
        (lossless.kind, lossless.language.as_deref(), lossless.codec.family, lossless.channels, lossless.flags),
        (StreamKind::Audio, Some("spa"), Family::DtsHdMa, Some(6), Flags(1024))
    );
    assert_eq!((lossless.bitrate_text.clone(), lossless.bitrate_kbps), (None, None));
    let core = &feature.streams[2];
    assert_eq!((core.codec.family, core.bitrate_kbps, core.flags), (Family::Dts, Some(1500), Flags(2304)));
    let english = &feature.streams[3];
    assert_eq!(
        (english.language.as_deref(), english.codec.family, english.channels, english.bitrate_kbps),
        (Some("eng"), Family::Ac3, Some(2), Some(192))
    );
    assert_eq!(english.layout_name.as_deref(), Some("stereo"));
    let forced: Vec<u32> =
        feature.streams.iter().filter(|stream| stream.flags.derived_forced_only()).map(|stream| stream.number).collect();
    assert_eq!(forced, [5, 7]);

    let menu = pans.title(1).unwrap();
    assert_eq!(menu.source(), TitleSource::File { name: "00055.mpls".to_string() });
    assert_eq!((menu.seconds, menu.chapters, menu.segment_count), (Some(340), 34, Some(34)));
    assert_eq!(menu.segments, (27..=60).collect::<Vec<i64>>());
    assert_eq!(menu.streams.len(), 1);
    assert!(same_scan_title(feature, feature));
    assert!(!same_scan_title(feature, menu));

    assert_eq!(recorded_bdmt_name(slug).as_deref(), Some("Pan's Labyrinth"));
}

#[test]
fn short_circuit_1986_dvd() {
    let slug = "short-circuit-1986-dvd";
    let Some(scan) = recorded_scan(slug) else { return };
    let dvd = typed_disc(slug, &scan);
    assert_eq!(dvd.kind, DiscKind::Dvd);
    assert!(dvd.name.is_some() && dvd.volume_label.is_some(), "CINFO 2 and 32 are present");
    assert_eq!(
        (dvd.name.as_deref(), dvd.tree_name.as_deref(), dvd.volume_label.as_deref()),
        (disc_attr(&scan, 2), disc_attr(&scan, 30), disc_attr(&scan, 32))
    );
    assert_eq!(dvd.metadata_language.as_deref(), disc_attr(&scan, 28));
    assert_eq!((dvd.makemkv_version.as_deref(), dvd.java_in_use), (Some("2.0.0"), false));
    assert_eq!((dvd.is_dvd, dvd.scan_read_errors), (true, 0));
    assert_eq!(dvd.titles.keys().copied().collect::<Vec<u32>>(), [0]);

    let main = dvd.title(0).unwrap();
    assert_eq!(main.source(), TitleSource::DvdTitle { number: 1 });
    assert_eq!((main.source_file.as_deref(), main.dvd_title.as_deref()), (None, Some("01")));
    assert!(main.comment.is_some(), "attribute 49 is present");
    assert_eq!(main.comment.as_deref(), title_attr(&scan, 0, 49));
    assert_eq!(main.seconds, Some(5904));
    assert_eq!((Some(u64::from(main.chapters)), main.size), (title_number(&scan, 0, 8), title_number(&scan, 0, 11)));
    let map = title_attr(&scan, 0, 26).unwrap();
    assert_eq!((main.segment_map.as_str(), main.segments.clone()), (map, segments(map)));
    assert_eq!(main.video_height(), Some(480));
    let kinds: Vec<StreamKind> = main.streams.iter().map(|stream| stream.kind).collect();
    assert_eq!(kinds, [StreamKind::Video, StreamKind::Audio, StreamKind::Audio]);
    let families: Vec<(Family, Option<u32>)> =
        main.streams.iter().map(|stream| (stream.codec.family, stream.channels)).collect();
    assert_eq!(families, [(Family::Mpeg2, None), (Family::Ac3, Some(6)), (Family::Ac3, Some(2))]);
    for stream in &main.streams {
        assert!(stream.bitrate_kbps.is_some(), "stream {}: attribute 13 is read", stream.number);
        assert_eq!(stream.bitrate_kbps, stream_kbps(&scan, 0, stream.number), "stream {}: attribute 13", stream.number);
    }
    assert!(main.streams_of(StreamKind::Subtitles).is_empty());
    assert!(main.streams.iter().all(|stream| stream.flags == Flags(0)));

    assert_eq!(recorded_bdmt_name(slug), None);
}

#[test]
fn short_circuit_2_1988_dvd() {
    let slug = "short-circuit-2-1988-dvd";
    let Some(scan) = recorded_scan(slug) else { return };
    let dvd = typed_disc(slug, &scan);
    assert_eq!(dvd.kind, DiscKind::Dvd);
    // The disc's name is the scan's CINFO 2, read when the test runs (RP21); its tree name is the same text, and its
    // volume label is the generic DVD_VIDEO the spec names.
    let name = disc_attr(&scan, 2).expect("CINFO 2 names the disc");
    assert_eq!(disc_attr(&scan, 30), Some(name));
    assert_eq!(
        (dvd.name.as_deref(), dvd.tree_name.as_deref(), dvd.volume_label.as_deref()),
        (Some(name), Some(name), Some("DVD_VIDEO"))
    );
    assert_eq!((dvd.makemkv_version.as_deref(), dvd.java_in_use), (Some("2.0.0"), false));
    assert_eq!((dvd.is_dvd, dvd.scan_read_errors), (true, 3));
    let sources: Vec<TitleSource> = dvd.titles.values().map(|title| title.source()).collect();
    assert_eq!(sources, [1, 3, 4, 5, 6, 7, 8].map(|number| TitleSource::DvdTitle { number }));
    for title in dvd.titles.values() {
        assert_eq!(title.seconds, title_seconds(&scan, title.number), "title {}: attribute 9", title.number);
    }

    let main = dvd.title(0).unwrap();
    assert!(main.comment.is_some(), "attribute 49 is present");
    assert_eq!((main.seconds, main.chapters, main.comment.as_deref()), (Some(6606), 28, title_attr(&scan, 0, 49)));
    let map = title_attr(&scan, 0, 26).unwrap();
    assert_eq!((main.segment_map.as_str(), main.segments.clone()), (map, segments(map)));
    let captions = &main.streams[12];
    assert_eq!(
        (captions.kind, captions.language.as_deref(), captions.codec.family),
        (StreamKind::Subtitles, Some("eng"), Family::ClosedCaptions)
    );
    assert_eq!(
        (captions.output_codec.as_deref(), captions.conversion.as_deref()),
        (Some("Text"), Some("( Lossy conversion )"))
    );
    assert_eq!(captions.tree_info.as_deref(), Some("CC\u{2192}Text English ( Lossy conversion )"));
    let vobsub = &main.streams[5];
    assert_eq!(
        (vobsub.codec.family, vobsub.codec_short.as_str(), vobsub.codec_long.as_str()),
        (Family::VobSub, "", "Dvd Subtitles")
    );

    // Titles 1 to 6 share one segment map; attribute 24 still tells every title apart.
    let shared = dvd.title(1).unwrap().segment_map.clone();
    assert!(!shared.is_empty());
    assert!((1..=6).all(|number| dvd.title(number).unwrap().segment_map == shared));
    for a in dvd.titles.values() {
        for b in dvd.titles.values() {
            assert_eq!(same_scan_title(a, b), a.number == b.number, "titles {} and {}", a.number, b.number);
        }
    }

    assert_eq!(recorded_bdmt_name(slug), None);
}

#[test]
fn the_office_2005_s3d3_bluray() {
    let slug = "the-office-2005-s3d3-bluray";
    let Some(scan) = recorded_scan(slug) else { return };
    let office = typed_disc(slug, &scan);
    assert_eq!(office.kind, DiscKind::BluRay);
    // The volume label is the scan's CINFO 32, read when the test runs (RP21); the name and the tree name are the same
    // text.
    let label = disc_attr(&scan, 32).expect("CINFO 32 is the volume label");
    assert_eq!((disc_attr(&scan, 2), disc_attr(&scan, 30)), (Some(label), Some(label)));
    assert_eq!(
        (office.name.as_deref(), office.tree_name.as_deref(), office.volume_label.as_deref()),
        (Some(label), Some(label), Some(label))
    );
    assert_eq!(office.metadata_language, None);
    assert_eq!((office.makemkv_version.as_deref(), office.java_in_use), (Some("2.0.0"), false));
    assert_eq!((office.is_dvd, office.scan_read_errors), (false, 0));
    assert_eq!(office.titles.keys().copied().collect::<Vec<u32>>(), (0..11).collect::<Vec<u32>>());
    for title in office.titles.values() {
        assert_eq!(title.seconds, title_seconds(&scan, title.number), "title {}: attribute 9", title.number);
    }

    let episode = office.title(1).unwrap();
    // Which playlist title 1 is stays in the clone (RP21): the source is attribute 16 of the same scan.
    let playlist = title_attr(&scan, 1, 16).expect("title 1 has attribute 16");
    assert_eq!(episode.source(), TitleSource::File { name: playlist.to_string() });
    assert_eq!(
        (Some(u64::from(episode.chapters)), Some(episode.segment_map.as_str())),
        (title_number(&scan, 1, 8), title_attr(&scan, 1, 26))
    );
    assert_eq!(episode.video_height(), Some(1080));
    let audio: Vec<(u32, Family, Flags)> = episode
        .streams_of(StreamKind::Audio)
        .iter()
        .map(|stream| (stream.number, stream.codec.family, stream.flags))
        .collect();
    assert_eq!(
        audio,
        [
            (1, Family::DtsHdMa, Flags(1024)),
            (2, Family::Dts, Flags(2304)),
            (3, Family::DtsHdMa, Flags(1024)),
            (4, Family::Dts, Flags(2304)),
        ]
    );
    let (twin_a, twin_b) = (&episode.streams[1], &episode.streams[3]);
    assert_eq!(
        (&twin_a.codec, twin_a.channels, twin_a.language.as_deref(), twin_a.tree_info.as_deref()),
        (&twin_b.codec, twin_b.channels, twin_b.language.as_deref(), twin_b.tree_info.as_deref())
    );

    // Titles 1, 2 and 6 run equally long and are still three titles.
    for (a, b) in [(1, 2), (1, 6), (2, 6)] {
        let (first, second) = (office.title(a).unwrap(), office.title(b).unwrap());
        assert!(first.seconds.is_some(), "title {a}: no duration");
        assert_eq!(first.seconds, second.seconds, "titles {a} and {b}: durations");
        assert!(!same_scan_title(first, second), "titles {a} and {b}");
    }
    let clip = office.title(7).unwrap();
    let clip_file = title_attr(&scan, 7, 16).unwrap();
    assert!(clip_file.ends_with(".m2ts"), "title 7 is a clip, not a playlist: {clip_file}");
    assert_eq!((clip.source(), clip.chapters), (TitleSource::File { name: clip_file.to_string() }, 0));
    // Title 3 holds two episodes (S03E12-E13): it lasts about twice title 1, within a tenth of title 1.
    let double = office.title(3).unwrap();
    assert_eq!(Some(double.segment_map.as_str()), title_attr(&scan, 3, 26));
    let (double_seconds, single_seconds) = (double.seconds.unwrap(), episode.seconds.unwrap());
    assert!(
        double_seconds.abs_diff(2 * single_seconds) * 10 < single_seconds,
        "title 3 runs {double_seconds} s, title 1 {single_seconds} s"
    );
    let two_clips = office.title(4).unwrap();
    assert_eq!(two_clips.segments, segments(title_attr(&scan, 4, 26).unwrap()));
    assert_eq!((two_clips.segment_count, two_clips.segments.len()), (Some(2), 2));

    assert_eq!(recorded_bdmt_name(slug), None);
}

#[test]
fn recordings_without_a_test_are_listed_as_not_covered() {
    let Some(slugs) = recorded_slugs() else { return };
    for slug in slugs.iter().filter(|slug| !RECORDINGS.contains(&slug.as_str())) {
        skip(&format!("not covered: {slug} has a scan.txt but no test in crates/dev/tests/disc_model.rs"));
    }
}
