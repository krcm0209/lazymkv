//! Track metadata in the final file (design spec 8.10): the file's tracks in order (video as the scan lists it,
//! audio in 8.6's order, subtitles forced, regular, SDH), each with its language tag and name and the four flags
//! `mkvmerge` sets (default, forced, hearing-impaired, original), every default flag explicit, the default-subtitle
//! rule, and chapters kept. Pure (RP19): it reads the scan's title and the decisions of Tasks 8 (audio) and 10
//! (subtitles). Written from the spec (RP20). Task 12's `verify_final` checks a partial file against the layout,
//! Task 13 puts it in the `FileDecision`, Task 16 prints it.

use crate::disc::{StreamKind, Title};
use crate::lang::is_english;
use crate::tracks::audio::AudioDecision;
use crate::tracks::subtitle::SubtitleDecision;
use crate::tracks::SubtitleRole;

/// One track of the final file (spec 8.10), with the values `mkvmerge` writes for it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutputTrack {
    /// The scan stream (0-based, within the title) the track comes from.
    pub stream: u32,
    /// Video, audio or subtitles.
    pub kind: StreamKind,
    /// The language tag (8.10): attribute 3 of the scan stream (for audio, the decision's copy of it); no tag for a stream
    /// with no code, an empty code or "und" (8.6 "Languages"; "-" counts as no code). "zxx", "mul" and "mis" keep their tag.
    pub language: Option<String>,
    /// The track name: None for video, the audio decision's name ("TrueHD Atmos 7.1"), the subtitle role's name
    /// ("English (Forced)", "English", "English (SDH)").
    pub name: Option<String>,
    /// The default flag, always written: the first video track, the audio decision's default track, and
    /// `default_subtitle`'s track.
    pub default: bool,
    /// The forced flag: the forced subtitle track only.
    pub forced: bool,
    /// The hearing-impaired flag: the SDH subtitle track only.
    pub hearing_impaired: bool,
    /// The original flag: the audio tracks 8.6 step 5 marks original.
    pub original: bool,
}

/// The final file's tracks and chapters (spec 8.10).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Layout {
    /// Video, then audio, then subtitles.
    pub tracks: Vec<OutputTrack>,
    /// 8.10 "Chapters are kept": always true.
    pub chapters_kept: bool,
}

/// Spec 8.10: the video streams as the scan lists them (a Dolby Vision enhancement layer kept as MakeMKV outputs it;
/// 3D video, attribute 6 containing "MVC", left out as the job profile leaves it out), the first with the default
/// flag; the audio tracks in the decision's order with its names and its default and original flags; the subtitle
/// tracks forced, regular, SDH (a stable sort by role), each tagged with attribute 3 of its scan stream (no tag for a
/// stream with no language, though 8.7 counted it as English: 8.10 "except streams with no language"), with their
/// names, forced and hearing-impaired flags, and the default flag on `default_subtitle`'s track. The default audio is the kept track flagged default, else the
/// first kept track; it is English when `lang::is_english` holds for its code, and a title without audio, or whose
/// default audio has no language, counts as not English. Every flag is explicit (8.10: MakeMKV sets its own);
/// chapters are kept.
pub fn layout(title: &Title, audio: &AudioDecision, subtitles: &SubtitleDecision) -> Layout {
    let mut tracks = Vec::new();
    let video = title.streams_of(StreamKind::Video).into_iter().filter(|stream| !stream.codec_short.contains("MVC"));
    for (position, stream) in video.enumerate() {
        tracks.push(OutputTrack {
            stream: stream.number,
            kind: StreamKind::Video,
            language: language_tag(stream.language.as_deref()),
            name: None,
            default: position == 0,
            forced: false,
            hearing_impaired: false,
            original: false,
        });
    }
    for track in &audio.kept {
        tracks.push(OutputTrack {
            stream: track.stream,
            kind: StreamKind::Audio,
            language: language_tag(track.language.as_deref()),
            name: Some(track.name.clone()),
            default: track.default,
            forced: false,
            hearing_impaired: false,
            original: track.original,
        });
    }
    let default = default_subtitle(default_audio_is_english(audio), subtitles);
    let mut kept: Vec<_> = subtitles.kept.iter().collect();
    kept.sort_by_key(|track| track.role);
    for track in kept {
        tracks.push(OutputTrack {
            stream: track.stream,
            kind: StreamKind::Subtitles,
            language: language_tag(
                title.streams.iter().find(|stream| stream.number == track.stream).and_then(|stream| stream.language.as_deref()),
            ),
            name: Some(track.name.clone()),
            default: default == Some(track.stream),
            forced: track.forced,
            hearing_impaired: track.hearing_impaired,
            original: false,
        });
    }
    Layout { tracks, chapters_kept: true }
}

/// Spec 8.10's default subtitle: the forced track when the default audio is English; when it is not, the regular
/// English track, or the SDH track when no regular track is kept (the forced track is then not default). 8.10 names
/// only the regular track; the SDH fallback serves the title whose only English dialogue track is SDH, and is a choice
/// the owner may overturn. None when the track the rule names does not exist.
pub fn default_subtitle(default_audio_is_english: bool, subtitles: &SubtitleDecision) -> Option<u32> {
    let kept = |role: SubtitleRole| subtitles.kept.iter().find(|track| track.role == role).map(|track| track.stream);
    if default_audio_is_english {
        kept(SubtitleRole::Forced)
    } else {
        kept(SubtitleRole::Regular).or_else(|| kept(SubtitleRole::Sdh))
    }
}

/// A stream's language tag: None for a stream with no language as 8.6 "Languages" defines it ("A stream with no code or
/// `und`"; an empty code or "-" counts as no code), else its code. Independent of Task 8's grouping by
/// `lang::is_no_language`: "zxx", "mul" and "mis" are ISO 639-2 codes `mkvmerge` accepts, and they keep their tag.
fn language_tag(code: Option<&str>) -> Option<String> {
    match code {
        None => None,
        Some(code) if code.is_empty() || code == "-" || code.eq_ignore_ascii_case("und") => None,
        Some(code) => Some(code.to_string()),
    }
}

/// Whether the file's default audio track is English: the kept track flagged default, else the first kept track.
fn default_audio_is_english(audio: &AudioDecision) -> bool {
    audio
        .kept
        .iter()
        .find(|track| track.default)
        .or_else(|| audio.kept.first())
        .and_then(|track| track.language.as_deref())
        .is_some_and(is_english)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::codec;
    use crate::disc::{Flags, Stream};
    use crate::tracks::audio::{AudioTrack, LangGroup};
    use crate::tracks::subtitle::{SubtitleCase, SubtitleTrack};
    use serde_json::Value;

    /// MakeMKV's attributes 5, 6 and 7 for the fixtures' codecs.
    const HEVC: (&str, &str, &str) = ("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1");
    const HEVC_ENHANCEMENT: (&str, &str, &str) =
        ("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1 (dvhe.07.06 BL+FEL+RPU)");
    const AVC: (&str, &str, &str) = ("V_MPEG4/ISO/AVC", "Mpeg4", "Mpeg4 AVC High@L4.1");
    const MVC: (&str, &str, &str) = ("V_MPEG4/ISO/MVC", "Mpeg4 MVC", "Mpeg4 MVC High@L4.1");
    const TRUEHD_ATMOS: (&str, &str, &str) = ("A_TRUEHD", "TrueHD", "TrueHD Atmos");
    const DTS_HD_MA: (&str, &str, &str) = ("A_DTS", "DTS-HD MA", "DTS-HD Master Audio");
    const AC3: (&str, &str, &str) = ("A_AC3", "DD", "Dolby Digital");
    const PGS: (&str, &str, &str) = ("S_HDMV/PGS", "PGS", "HDMV PGS Subtitles");

    /// A scan stream with only the attributes the layout reads (kind, language, codec).
    fn stream(number: u32, kind: StreamKind, language: Option<&str>, (id, short, long): (&str, &str, &str)) -> Stream {
        Stream {
            number,
            kind,
            name: None,
            language: language.map(str::to_string),
            codec: codec(id, short, long),
            codec_id: id.to_string(),
            codec_short: short.to_string(),
            codec_long: long.to_string(),
            bitrate_text: None,
            bitrate_kbps: None,
            channels: None,
            sample_rate: None,
            sample_size: None,
            video_size: None,
            flags: Flags::default(),
            tree_info: None,
            layout_name: None,
            output_codec: None,
            conversion: None,
            file_codec: codec(id, short, long),
        }
    }

    /// Title 0 with `chapters` chapters and `streams`.
    fn title(chapters: u32, streams: Vec<Stream>) -> Title {
        Title {
            number: 0,
            name: None,
            chapters,
            seconds: Some(7140),
            size: None,
            source_file: Some("00800.mpls".to_string()),
            dvd_title: None,
            segment_count: None,
            segment_map: String::new(),
            segments: Vec::new(),
            truncated: false,
            output_name: None,
            comment: None,
            streams,
        }
    }

    /// A kept audio track named `name`: the layout copies the name the audio decision gives (Task 8 builds it as 8.10's
    /// "<display>[ <Atmos|DTS:X>] <layout>" and tests that rule), so the fixtures give it.
    #[allow(clippy::too_many_arguments)]
    fn audio_track(
        stream: u32,
        language: Option<&str>,
        group: LangGroup,
        (id, short, long): (&str, &str, &str),
        channels: u32,
        name: &str,
        default: bool,
        original: bool,
    ) -> AudioTrack {
        let codec = codec(id, short, long);
        let object = codec.object;
        AudioTrack {
            stream,
            language: language.map(str::to_string),
            group,
            file_codec: codec.clone(),
            codec,
            channels,
            tier: None,
            object,
            sample_size: None,
            sample_rate: None,
            bitrate_kbps: None,
            first_of_language: true,
            name: name.to_string(),
            default,
            original,
        }
    }

    /// An audio decision keeping `kept` in that order.
    fn audio(kept: Vec<AudioTrack>) -> AudioDecision {
        let reference = kept.first().map(|track| track.stream);
        AudioDecision {
            kept,
            dropped: Vec::new(),
            reference,
            verdict_reference: None,
            notes: Vec::new(),
            reviews: Vec::new(),
            confidence: 0.95,
            description_alignment: None,
        }
    }

    /// A kept subtitle track in `role`, flagged and named as 8.7 says.
    fn subtitle(stream: u32, role: SubtitleRole) -> SubtitleTrack {
        SubtitleTrack {
            stream,
            role,
            name: role.name().to_string(),
            forced: role == SubtitleRole::Forced,
            hearing_impaired: role == SubtitleRole::Sdh,
            default: false,
            events: Some(if role == SubtitleRole::Forced { 40 } else { 1200 }),
            closed_captions: false,
            parent: None,
            decided_by: "count".to_string(),
        }
    }

    /// A subtitle decision keeping `kept` in that order.
    fn subtitles(kept: Vec<SubtitleTrack>) -> SubtitleDecision {
        SubtitleDecision {
            kept,
            dropped: Vec::new(),
            set_aside: Vec::new(),
            full: Vec::new(),
            groups: Vec::new(),
            dialogue_group: None,
            dialogue_pick: None,
            pairs: Vec::new(),
            case: SubtitleCase::Alignment,
            confidence: 0.85,
            notes: Vec::new(),
            reviews: Vec::new(),
        }
    }

    /// A Spanish film: video, Spanish TrueHD Atmos, English DTS-HD MA and Dolby Digital, three English PGS streams.
    fn film_title() -> Title {
        title(
            16,
            vec![
                stream(0, StreamKind::Video, None, HEVC),
                stream(1, StreamKind::Audio, Some("spa"), TRUEHD_ATMOS),
                stream(2, StreamKind::Audio, Some("eng"), DTS_HD_MA),
                stream(3, StreamKind::Audio, Some("eng"), AC3),
                stream(4, StreamKind::Subtitles, Some("eng"), PGS),
                stream(5, StreamKind::Subtitles, Some("eng"), PGS),
                stream(6, StreamKind::Subtitles, Some("eng"), PGS),
            ],
        )
    }

    /// The film's audio as 8.6 step 5 orders it with original_language_first on: Spanish first (default, original),
    /// then English.
    fn film_audio() -> AudioDecision {
        audio(vec![
            audio_track(1, Some("spa"), LangGroup::Original, TRUEHD_ATMOS, 8, "TrueHD Atmos 7.1", true, true),
            audio_track(2, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "DTS-HD MA 5.1", false, false),
            audio_track(3, Some("eng"), LangGroup::English, AC3, 2, "Dolby Digital 2.0", false, false),
        ])
    }

    /// The film's subtitles: stream 5 forced, 4 regular, 6 SDH.
    fn film_subtitles() -> SubtitleDecision {
        subtitles(vec![
            subtitle(5, SubtitleRole::Forced),
            subtitle(4, SubtitleRole::Regular),
            subtitle(6, SubtitleRole::Sdh),
        ])
    }

    /// One English audio track at `stream`, default and original (an English work).
    fn english_audio(stream: u32) -> AudioDecision {
        audio(vec![audio_track(stream, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "DTS-HD MA 5.1", true, true)])
    }

    /// (stream, default) of a layout's subtitle tracks, in file order.
    fn subtitle_defaults(layout: &Layout) -> Vec<(u32, bool)> {
        layout
            .tracks
            .iter()
            .filter(|track| track.kind == StreamKind::Subtitles)
            .map(|track| (track.stream, track.default))
            .collect()
    }

    #[test]
    fn with_english_default_audio_the_forced_track_is_the_default_subtitle() {
        assert_eq!(default_subtitle(true, &film_subtitles()), Some(5));
    }

    #[test]
    fn with_other_default_audio_the_regular_track_is_the_default_subtitle() {
        assert_eq!(default_subtitle(false, &film_subtitles()), Some(4));
    }

    #[test]
    fn with_english_audio_and_no_forced_track_no_subtitle_is_default() {
        let kept = subtitles(vec![subtitle(4, SubtitleRole::Regular), subtitle(6, SubtitleRole::Sdh)]);
        assert_eq!(default_subtitle(true, &kept), None);
    }

    #[test]
    fn with_other_audio_and_no_regular_track_the_sdh_track_is_the_default_subtitle() {
        let kept = subtitles(vec![subtitle(5, SubtitleRole::Forced), subtitle(6, SubtitleRole::Sdh)]);
        assert_eq!(default_subtitle(false, &kept), Some(6));
    }

    #[test]
    fn with_other_audio_and_only_a_forced_track_no_subtitle_is_default() {
        let kept = subtitles(vec![subtitle(5, SubtitleRole::Forced)]);
        assert_eq!(default_subtitle(false, &kept), None);
    }

    #[test]
    fn without_a_kept_subtitle_track_there_is_no_default_subtitle() {
        assert_eq!(default_subtitle(true, &subtitles(Vec::new())), None);
        assert_eq!(default_subtitle(false, &subtitles(Vec::new())), None);
    }

    #[test]
    fn the_file_holds_video_then_audio_then_subtitles_with_every_flag_set() {
        let eng = Some("eng".to_string());
        let expected = Layout {
            tracks: vec![
                OutputTrack {
                    stream: 0,
                    kind: StreamKind::Video,
                    language: None,
                    name: None,
                    default: true,
                    forced: false,
                    hearing_impaired: false,
                    original: false,
                },
                OutputTrack {
                    stream: 1,
                    kind: StreamKind::Audio,
                    language: Some("spa".to_string()),
                    name: Some("TrueHD Atmos 7.1".to_string()),
                    default: true,
                    forced: false,
                    hearing_impaired: false,
                    original: true,
                },
                OutputTrack {
                    stream: 2,
                    kind: StreamKind::Audio,
                    language: eng.clone(),
                    name: Some("DTS-HD MA 5.1".to_string()),
                    default: false,
                    forced: false,
                    hearing_impaired: false,
                    original: false,
                },
                OutputTrack {
                    stream: 3,
                    kind: StreamKind::Audio,
                    language: eng.clone(),
                    name: Some("Dolby Digital 2.0".to_string()),
                    default: false,
                    forced: false,
                    hearing_impaired: false,
                    original: false,
                },
                OutputTrack {
                    stream: 5,
                    kind: StreamKind::Subtitles,
                    language: eng.clone(),
                    name: Some("English (Forced)".to_string()),
                    default: false,
                    forced: true,
                    hearing_impaired: false,
                    original: false,
                },
                OutputTrack {
                    stream: 4,
                    kind: StreamKind::Subtitles,
                    language: eng.clone(),
                    name: Some("English".to_string()),
                    default: true,
                    forced: false,
                    hearing_impaired: false,
                    original: false,
                },
                OutputTrack {
                    stream: 6,
                    kind: StreamKind::Subtitles,
                    language: eng,
                    name: Some("English (SDH)".to_string()),
                    default: false,
                    forced: false,
                    hearing_impaired: true,
                    original: false,
                },
            ],
            chapters_kept: true,
        };
        assert_eq!(layout(&film_title(), &film_audio(), &film_subtitles()), expected);
    }

    #[test]
    fn every_video_stream_is_kept_in_scan_order_and_only_the_first_is_default() {
        let two_layers = title(
            16,
            vec![
                stream(0, StreamKind::Video, None, HEVC),
                stream(1, StreamKind::Video, None, HEVC_ENHANCEMENT),
                stream(2, StreamKind::Audio, Some("eng"), DTS_HD_MA),
            ],
        );
        let got = layout(&two_layers, &english_audio(2), &subtitles(Vec::new()));
        let video: Vec<(u32, bool, Option<String>, Option<String>)> = got
            .tracks
            .iter()
            .filter(|track| track.kind == StreamKind::Video)
            .map(|track| (track.stream, track.default, track.language.clone(), track.name.clone()))
            .collect();
        assert_eq!(video, vec![(0, true, None, None), (1, false, None, None)]);
    }

    #[test]
    fn three_d_video_is_left_out() {
        let three_d = title(
            12,
            vec![
                stream(0, StreamKind::Video, None, AVC),
                stream(1, StreamKind::Video, None, MVC),
                stream(2, StreamKind::Audio, Some("eng"), AC3),
            ],
        );
        let kept = audio(vec![
            audio_track(2, Some("eng"), LangGroup::English, AC3, 6, "Dolby Digital 5.1", true, true),
        ]);
        let got = layout(&three_d, &kept, &subtitles(Vec::new()));
        let streams: Vec<(u32, StreamKind)> = got.tracks.iter().map(|track| (track.stream, track.kind)).collect();
        assert_eq!(streams, vec![(0, StreamKind::Video), (2, StreamKind::Audio)]);
    }

    #[test]
    fn audio_keeps_the_decisions_order_and_flags() {
        let kept = audio(vec![
            audio_track(2, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "DTS-HD MA 5.1", true, true),
            audio_track(3, Some("eng"), LangGroup::English, AC3, 2, "Dolby Digital 2.0", false, true),
            audio_track(1, Some("und"), LangGroup::NoLanguage, AC3, 2, "Dolby Digital 2.0", false, false),
        ]);
        let got = layout(&film_title(), &kept, &subtitles(Vec::new()));
        let flags: Vec<(u32, bool, bool, bool, bool)> = got
            .tracks
            .iter()
            .filter(|track| track.kind == StreamKind::Audio)
            .map(|track| (track.stream, track.default, track.original, track.forced, track.hearing_impaired))
            .collect();
        assert_eq!(flags, vec![(2, true, true, false, false), (3, false, true, false, false), (1, false, false, false, false)]);
    }

    #[test]
    fn audio_names_are_carried_unchanged_from_the_audio_decision() {
        let kept = audio(vec![
            audio_track(1, Some("spa"), LangGroup::Original, TRUEHD_ATMOS, 8, "Spanish Atmos Mix", true, true),
            audio_track(2, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "Custom Name 2.0", false, false),
            audio_track(3, Some("eng"), LangGroup::English, AC3, 2, "TrueHD Atmos 7.1", false, false),
        ]);
        let names: Vec<(u32, Option<String>)> = layout(&film_title(), &kept, &subtitles(Vec::new()))
            .tracks
            .into_iter()
            .filter(|track| track.kind == StreamKind::Audio)
            .map(|track| (track.stream, track.name))
            .collect();
        assert_eq!(
            names,
            vec![
                (1, Some("Spanish Atmos Mix".to_string())),
                (2, Some("Custom Name 2.0".to_string())),
                (3, Some("TrueHD Atmos 7.1".to_string())),
            ]
        );
    }

    #[test]
    fn a_no_language_audio_track_has_no_language_tag() {
        let kept = audio(vec![
            audio_track(1, Some("spa"), LangGroup::Original, AC3, 6, "Dolby Digital 5.1", true, true),
            audio_track(2, Some("und"), LangGroup::NoLanguage, AC3, 2, "Dolby Digital 2.0", false, false),
            audio_track(3, None, LangGroup::NoLanguage, AC3, 2, "Dolby Digital 2.0", false, false),
            audio_track(4, Some("zxx"), LangGroup::NoLanguage, AC3, 2, "Dolby Digital 2.0", false, false),
            audio_track(5, Some("mul"), LangGroup::NoLanguage, AC3, 2, "Dolby Digital 2.0", false, false),
            audio_track(6, Some(""), LangGroup::NoLanguage, AC3, 2, "Dolby Digital 2.0", false, false),
        ]);
        let languages: Vec<Option<String>> = layout(&film_title(), &kept, &subtitles(Vec::new()))
            .tracks
            .into_iter()
            .filter(|track| track.kind == StreamKind::Audio)
            .map(|track| track.language)
            .collect();
        assert_eq!(
            languages,
            vec![Some("spa".to_string()), None, None, Some("zxx".to_string()), Some("mul".to_string()), None]
        );
    }

    #[test]
    fn subtitles_are_ordered_forced_regular_sdh_with_their_own_language_tag() {
        let mut undetermined = film_title();
        undetermined.streams[4].language = Some("und".to_string());
        let shuffled = subtitles(vec![
            subtitle(6, SubtitleRole::Sdh),
            subtitle(4, SubtitleRole::Regular),
            subtitle(5, SubtitleRole::Forced),
        ]);
        let got = layout(&undetermined, &english_audio(2), &shuffled);
        let eng = Some("eng".to_string());
        let subs: Vec<(u32, Option<String>, Option<String>, bool, bool)> = got
            .tracks
            .iter()
            .filter(|track| track.kind == StreamKind::Subtitles)
            .map(|track| (track.stream, track.language.clone(), track.name.clone(), track.forced, track.hearing_impaired))
            .collect();
        assert_eq!(
            subs,
            vec![
                (5, eng.clone(), Some("English (Forced)".to_string()), true, false),
                (4, None, Some("English".to_string()), false, false),
                (6, eng, Some("English (SDH)".to_string()), false, true),
            ]
        );
        assert!(got.tracks.iter().all(|track| !track.original || track.kind == StreamKind::Audio));
    }

    #[test]
    fn with_english_default_audio_the_forced_subtitle_is_flagged_default() {
        let got = layout(&film_title(), &english_audio(2), &film_subtitles());
        assert_eq!(subtitle_defaults(&got), vec![(5, true), (4, false), (6, false)]);
    }

    #[test]
    fn with_non_english_default_audio_the_regular_subtitle_is_flagged_default_and_the_forced_is_not() {
        let got = layout(&film_title(), &film_audio(), &film_subtitles());
        assert_eq!(subtitle_defaults(&got), vec![(5, false), (4, true), (6, false)]);
    }

    #[test]
    fn a_no_language_default_audio_counts_as_not_english() {
        for language in [Some("und"), None] {
            let kept = audio(vec![
                audio_track(2, language, LangGroup::NoLanguage, AC3, 2, "Dolby Digital 2.0", true, false),
            ]);
            let got = layout(&film_title(), &kept, &film_subtitles());
            assert_eq!(subtitle_defaults(&got), vec![(5, false), (4, true), (6, false)], "{language:?}");
        }
    }

    #[test]
    fn without_audio_the_regular_subtitle_is_flagged_default() {
        let got = layout(&film_title(), &audio(Vec::new()), &film_subtitles());
        assert_eq!(subtitle_defaults(&got), vec![(5, false), (4, true), (6, false)]);
    }

    #[test]
    fn without_a_flagged_default_audio_the_first_audio_track_decides() {
        let english_first = audio(vec![
            audio_track(2, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "DTS-HD MA 5.1", false, true),
            audio_track(1, Some("spa"), LangGroup::Original, TRUEHD_ATMOS, 8, "TrueHD Atmos 7.1", false, false),
        ]);
        let got = layout(&film_title(), &english_first, &film_subtitles());
        assert_eq!(subtitle_defaults(&got), vec![(5, true), (4, false), (6, false)]);
        let spanish_first = audio(vec![
            audio_track(1, Some("spa"), LangGroup::Original, TRUEHD_ATMOS, 8, "TrueHD Atmos 7.1", false, true),
            audio_track(2, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "DTS-HD MA 5.1", false, false),
        ]);
        let got = layout(&film_title(), &spanish_first, &film_subtitles());
        assert_eq!(subtitle_defaults(&got), vec![(5, false), (4, true), (6, false)]);
    }

    #[test]
    fn the_default_audio_is_the_flagged_track_even_when_it_is_not_first() {
        let english_flagged = audio(vec![
            audio_track(1, Some("spa"), LangGroup::Original, TRUEHD_ATMOS, 8, "TrueHD Atmos 7.1", false, true),
            audio_track(2, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "DTS-HD MA 5.1", true, false),
        ]);
        let got = layout(&film_title(), &english_flagged, &film_subtitles());
        assert_eq!(subtitle_defaults(&got), vec![(5, true), (4, false), (6, false)]);
        let spanish_flagged = audio(vec![
            audio_track(2, Some("eng"), LangGroup::English, DTS_HD_MA, 6, "DTS-HD MA 5.1", false, false),
            audio_track(1, Some("spa"), LangGroup::Original, TRUEHD_ATMOS, 8, "TrueHD Atmos 7.1", true, true),
        ]);
        let got = layout(&film_title(), &spanish_flagged, &film_subtitles());
        assert_eq!(subtitle_defaults(&got), vec![(5, false), (4, true), (6, false)]);
    }

    #[test]
    fn every_track_carries_an_explicit_default_flag() {
        let value = serde_json::to_value(layout(&film_title(), &film_audio(), &film_subtitles())).unwrap();
        let tracks = value["tracks"].as_array().unwrap();
        assert_eq!(tracks.len(), 7);
        for track in tracks {
            assert!(matches!(track["default"], Value::Bool(_)), "{track}");
        }
        let defaults = tracks.iter().filter(|track| track["default"] == Value::Bool(true)).count();
        assert_eq!(defaults, 3);
    }

    #[test]
    fn chapters_are_kept() {
        assert!(layout(&film_title(), &film_audio(), &film_subtitles()).chapters_kept);
        let unchaptered = title(0, vec![stream(0, StreamKind::Video, None, HEVC)]);
        assert!(layout(&unchaptered, &audio(Vec::new()), &subtitles(Vec::new())).chapters_kept);
    }

    #[test]
    fn the_layout_serialises_with_its_field_order_and_round_trips() {
        let one = Layout {
            tracks: vec![OutputTrack {
                stream: 4,
                kind: StreamKind::Subtitles,
                language: Some("eng".to_string()),
                name: Some("English (SDH)".to_string()),
                default: false,
                forced: false,
                hearing_impaired: true,
                original: false,
            }],
            chapters_kept: true,
        };
        let text = serde_json::to_string(&one).unwrap();
        assert_eq!(
            text,
            r#"{"tracks":[{"stream":4,"kind":"subtitles","language":"eng","name":"English (SDH)","default":false,"forced":false,"hearing_impaired":true,"original":false}],"chapters_kept":true}"#
        );
        assert_eq!(serde_json::from_str::<Layout>(&text).unwrap(), one);
    }
}
