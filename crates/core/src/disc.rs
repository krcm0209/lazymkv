//! The typed disc model of spec 9.4 ("Output parsing": the attribute ids and value formats) and 9.5 ("Stream flags"):
//! `disc` turns a parsed robot-mode scan (`robot::Scan`) into a `Disc` of typed `Title`s and `Stream`s, with the
//! MakeMKV version banner (MSG:1005) and the Java runtime message (MSG:3344) that spec 6.1 step 4 records. It also holds
//! the title matching that spec 5 ("titles are matched by source file, segment map and duration within 2 s"), 6.3 step 8
//! and 8.3's first paragraph share (`TitleSource`, `title_source`, `same_source`, `segment_maps_agree`,
//! `DURATION_MATCH_S`, `same_scan_title`), and the disc-name reader of spec 6.1 step 2 over the text of
//! `BDMV/META/DL/bdmt_*.xml` (`bdmt_name`, `bdmt_candidates`). No I/O: callers hand in the parsed scan and the XML text.
//! Used by Tasks 3 to 16 of the phase 1 plan.

use std::collections::{BTreeMap, BTreeSet};

use crate::audio::titles::{dvd_scan, segments};
use crate::codec::{bitrate_kbps, codec, output_codec, Codec, Family};
use crate::robot::{duration_seconds, int_attr, Attrs, Scan};
use crate::settings::version_triple;

/// Attribute 1 of a stream: 6201 "Video", 6202 "Audio", 6203 "Subtitles". Spec 9.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamKind {
    Video,
    Audio,
    Subtitles,
}

impl StreamKind {
    /// "video", "audio" or "subtitles": the serde spelling. Spec 9.4.
    pub fn as_str(self) -> &'static str {
        match self {
            StreamKind::Video => "video",
            StreamKind::Audio => "audio",
            StreamKind::Subtitles => "subtitles",
        }
    }

    /// The kind attribute 1's text names, spelt as MakeMKV writes it ("Video", "Audio", "Subtitles"); None for any other
    /// text. Spec 9.4.
    pub fn from_attr(text: &str) -> Option<StreamKind> {
        match text {
            "Video" => Some(StreamKind::Video),
            "Audio" => Some(StreamKind::Audio),
            "Subtitles" => Some(StreamKind::Subtitles),
            _ => None,
        }
    }
}

/// Attribute 22 as a bitmask (spec 9.5). Unknown bits are kept in the value and ignored by every query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct Flags(pub u32);

impl Flags {
    /// Spec 9.5: director's comments.
    pub const DIRECTORS_COMMENTS: u32 = 1;
    /// Spec 9.5: alternate director's comments.
    pub const ALT_DIRECTORS_COMMENTS: u32 = 2;
    /// Spec 9.5: visually impaired.
    pub const VISUALLY_IMPAIRED: u32 = 4;
    /// Spec 9.5: core audio.
    pub const CORE_AUDIO: u32 = 256;
    /// Spec 9.5: secondary audio.
    pub const SECONDARY_AUDIO: u32 = 512;
    /// Spec 9.5: has core audio.
    pub const HAS_CORE_AUDIO: u32 = 1024;
    /// Spec 9.5: derived stream. The same bit as audio::labels::DERIVED (pinned by a test: `Flags::DERIVED as i64 ==
    /// audio::labels::DERIVED`).
    pub const DERIVED: u32 = 2048;
    /// Spec 9.5: forced subtitles.
    pub const FORCED: u32 = 4096;
    /// The bits 9.5 names; every other bit (16384, 32768, 65536 seen) is ignored.
    pub const KNOWN: u32 = 1 | 2 | 4 | 256 | 512 | 1024 | 2048 | 4096;

    /// `robot::int_attr(attrs, 22)` as Flags (0 for an absent or empty attribute); Err with int_attr's text when attribute 22
    /// is not a number (never seen), and Err `attribute 22 is <n>, outside the flag range 0 to 4294967295` when it is
    /// negative or above u32::MAX. Spec 9.5.
    pub fn of(attrs: &Attrs) -> Result<Flags, String> {
        let value = int_attr(attrs, 22)?;
        u32::try_from(value)
            .map(Flags)
            .map_err(|_| format!("attribute 22 is {value}, outside the flag range 0 to {}", u32::MAX))
    }

    /// True when `bit` is one or more known bits and every one of them is set; an argument holding a bit outside KNOWN
    /// never holds, and neither does 0. Spec 9.5.
    pub fn has(self, bit: u32) -> bool {
        bit != 0 && bit & !Flags::KNOWN == 0 && self.0 & bit == bit
    }

    /// Flags 1, 2 or 4 (8.6 step 1's "director's comments, alternate director's comments or visually impaired"). Spec 9.5.
    pub fn commentary(self) -> bool {
        self.0 & (Flags::DIRECTORS_COMMENTS | Flags::ALT_DIRECTORS_COMMENTS | Flags::VISUALLY_IMPAIRED) != 0
    }

    /// 4096 and 2048 together (8.7 "derived forced-only"). Spec 9.5.
    pub fn derived_forced_only(self) -> bool {
        self.has(Flags::FORCED | Flags::DERIVED)
    }

    /// 4096 without 2048 (8.7 "a DVD stream flagged forced"). Spec 9.5.
    pub fn forced_authored(self) -> bool {
        self.has(Flags::FORCED) && !self.has(Flags::DERIVED)
    }

    /// The known bits only. Spec 9.5.
    pub fn known(self) -> u32 {
        self.0 & Flags::KNOWN
    }
}

/// How the matching rules know a title (spec 5, 6.2 step 6, 6.3 step 8, 8.3): its source file (attribute 16) on a Blu-ray,
/// else its DVD title number (attribute 24 parsed as a number: "01" and "1" are equal; TheDiscDB writes the same number in
/// its `source_file` on a DVD), else Unknown.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "by")]
pub enum TitleSource {
    /// Attribute 16 as the scan wrote it ("00800.mpls", "00007.m2ts").
    File { name: String },
    /// Attribute 24 as a number.
    DvdTitle { number: u32 },
    /// Neither: such a title matches nothing.
    Unknown,
}

/// `source_file` when Some and not empty, else `dvd_title` parsed as a number (surrounding whitespace ignored), else
/// Unknown. Spec 5, 6.2 step 6.
pub fn title_source(source_file: Option<&str>, dvd_title: Option<&str>) -> TitleSource {
    match source_file {
        Some(name) if !name.is_empty() => TitleSource::File { name: name.to_string() },
        _ => match dvd_title.and_then(|text| text.trim().parse::<u32>().ok()) {
            Some(number) => TitleSource::DvdTitle { number },
            None => TitleSource::Unknown,
        },
    }
}

/// Equal and not Unknown (an Unknown never matches anything). Spec 5, 8.3.
pub fn same_source(a: &TitleSource, b: &TitleSource) -> bool {
    *a != TitleSource::Unknown && a == b
}

/// Spec 14's "2-second duration match": the one copy for every new matcher (8.3, 5, 6.3 step 8, 6.2 step 7); equal to
/// reconcile::DURATION_TOLERANCE_S (pinned by a test).
pub const DURATION_MATCH_S: u32 = 2;

/// Spec 8.3 first paragraph: two segment maps agree when equal; when either ends in "..." only the untruncated prefix is
/// compared (the prefix is the text before "...", with a trailing partial number or comma dropped), and the other map must
/// start with it. Parts are compared whole; of two truncated maps, the shorter known prefix must start the longer.
pub fn segment_maps_agree(a: &str, b: &str) -> bool {
    let (a_parts, a_truncated) = known_parts(a);
    let (b_parts, b_truncated) = known_parts(b);
    match (a_truncated, b_truncated) {
        (false, false) => a == b,
        (true, false) => b_parts.starts_with(&a_parts),
        (false, true) => a_parts.starts_with(&b_parts),
        (true, true) => {
            if a_parts.len() <= b_parts.len() {
                b_parts.starts_with(&a_parts)
            } else {
                a_parts.starts_with(&b_parts)
            }
        }
    }
}

/// A segment map's parts that are known whole, and whether the map was truncated: for a map ending in "...", the parts
/// before the dots without the last one unless the text before the dots ends in a comma (that last part may be a number
/// MakeMKV cut). Spec 8.3.
fn known_parts(map: &str) -> (Vec<&str>, bool) {
    match map.strip_suffix("...") {
        None => (clip_parts(map), false),
        Some(body) => {
            let mut parts = clip_parts(body);
            if !body.trim_end().ends_with(',') {
                parts.pop();
            }
            (parts, true)
        }
    }
}

/// The comma-separated parts of a segment map, trimmed, empty parts dropped.
fn clip_parts(text: &str) -> Vec<&str> {
    text.split(',').map(str::trim).filter(|part| !part.is_empty()).collect()
}

/// One stream of a title, typed (spec 9.4's attribute list).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stream {
    /// The scan's stream number (0-based, within the title).
    pub number: u32,
    /// Attribute 1.
    pub kind: StreamKind,
    /// Attribute 2 ("Surround 5.1", "Stereo"); None when absent or empty.
    pub name: Option<String>,
    /// Attribute 3 lower-cased; None when absent or empty (a video stream carries none). "und" stays "und".
    pub language: Option<String>,
    /// `codec::codec(attribute 5, 6, 7)`.
    pub codec: Codec,
    /// Attributes 5, 6, 7 as the scan wrote them ("" when absent). `codec_long` is what TheDiscDB's track `name` holds
    /// ("DTS-HD Master Audio", "TrueHD Atmos", "Dvd Subtitles"), so 8.6 step 1's alignment compares it.
    pub codec_id: String,
    pub codec_short: String,
    pub codec_long: String,
    /// Attribute 13 as text, and `codec::bitrate_kbps` of it.
    pub bitrate_text: Option<String>,
    pub bitrate_kbps: Option<u32>,
    /// Attribute 14.
    pub channels: Option<u32>,
    /// Attributes 17 and 18.
    pub sample_rate: Option<u32>,
    pub sample_size: Option<u32>,
    /// Attribute 19 "WxH" as (width, height).
    pub video_size: Option<(u32, u32)>,
    /// Attribute 22 (`Flags::of`).
    pub flags: Flags,
    /// Attribute 30 ("PGS English  (forced only)", "CC->Text English ( Lossy conversion )").
    pub tree_info: Option<String>,
    /// Attribute 40 ("stereo", "5.1(side)", "7.1").
    pub layout_name: Option<String>,
    /// Attribute 41, MakeMKV's output codec short name (apdefs.h `ap_iaOutputCodecShort`, read by
    /// `codec::output_codec`; recorded only on a converted stream: "Text"), and 42 ("( Lossless conversion )", "( Lossy
    /// conversion )").
    pub output_codec: Option<String>,
    pub conversion: Option<String>,
    /// The codec the rip's file holds: `codec::output_codec` of attribute 41 when present (an LPCM stream MakeMKV writes
    /// as FLAC is FLAC, a DVD's closed captions converted to text are text), else `codec`. Read once here, so 6.2 step
    /// 7's verification and 8.10's audio names agree. Spec 6.2 step 7, 8.10, 9.4.
    pub file_codec: Codec,
}

/// One title of the scan, typed. Spec 9.4.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Title {
    /// The scan's title number (sparse numbering kept).
    pub number: u32,
    /// Attribute 2.
    pub name: Option<String>,
    /// Attribute 8; 0 when absent.
    pub chapters: u32,
    /// Attribute 9 through `robot::duration_seconds`; None when absent or unparsable.
    pub seconds: Option<u32>,
    /// Attribute 11, bytes.
    pub size: Option<u64>,
    /// Attribute 16 (a Blu-ray's playlist, "00800.mpls"); a DVD has none.
    pub source_file: Option<String>,
    /// Attribute 24 as text ("01"), the DVD title number.
    pub dvd_title: Option<String>,
    /// Attribute 25.
    pub segment_count: Option<u32>,
    /// Attribute 26 as text ("" when absent) and `audio::titles::segments` of it; `truncated` when the text ends in "...".
    pub segment_map: String,
    pub segments: Vec<i64>,
    pub truncated: bool,
    /// Attribute 27, MakeMKV's output file name.
    pub output_name: Option<String>,
    /// Attribute 49 ("FPL_MainFeature_UR", "SEG FPL_02", "A1").
    pub comment: Option<String>,
    /// The streams in scan order; a stream whose attribute 1 names no kind is left out (no recorded or pinned scan has
    /// one).
    pub streams: Vec<Stream>,
}

impl Title {
    /// The streams of one kind, in scan order. Spec 9.4.
    pub fn streams_of(&self, kind: StreamKind) -> Vec<&Stream> {
        self.streams.iter().filter(|stream| stream.kind == kind).collect()
    }

    /// The height of the first video stream's attribute 19 (8.9's resolution label); None when the title has no video
    /// stream or its first one has no size.
    pub fn video_height(&self) -> Option<u32> {
        self.streams
            .iter()
            .find(|stream| stream.kind == StreamKind::Video)
            .and_then(|stream| stream.video_size)
            .map(|(_, height)| height)
    }

    /// Minutes as f64 (seconds / 60.0); None without a duration. Spec 8.3.
    pub fn minutes(&self) -> Option<f64> {
        self.seconds.map(|seconds| f64::from(seconds) / 60.0)
    }

    /// The (kind, language, codec family, channels) of every audio and subtitle stream as a set, for 8.3 rule 3.4's
    /// "superset of every other title's" test.
    pub fn signature_set(&self) -> BTreeSet<(StreamKind, Option<String>, Family, Option<u32>)> {
        self.streams
            .iter()
            .filter(|stream| stream.kind != StreamKind::Video)
            .map(|stream| (stream.kind, stream.language.clone(), stream.codec.family, stream.channels))
            .collect()
    }

    /// `title_source(source_file, dvd_title)`. Spec 5.
    pub fn source(&self) -> TitleSource {
        title_source(self.source_file.as_deref(), self.dvd_title.as_deref())
    }
}

/// CINFO 1: 6209 "Blu-ray disc", 6206 "DVD disc"; a UHD is a Blu-ray disc here (the resolution tells them apart, 8.9).
/// Spec 9.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiscKind {
    BluRay,
    Dvd,
    Unknown,
}

impl DiscKind {
    /// "blu-ray", "dvd" or "unknown": the serde spelling. Spec 9.4.
    pub fn as_str(self) -> &'static str {
        match self {
            DiscKind::BluRay => "blu-ray",
            DiscKind::Dvd => "dvd",
            DiscKind::Unknown => "unknown",
        }
    }
}

/// The typed disc (spec 9.4 CINFO and the two messages of 6.1 step 4).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Disc {
    pub kind: DiscKind,
    /// CINFO 2 (the disc's name, "Pan's Labyrinth").
    pub name: Option<String>,
    /// CINFO 30.
    pub tree_name: Option<String>,
    /// CINFO 32 ("PANS_LABYRINTH", "DVD_VIDEO").
    pub volume_label: Option<String>,
    /// CINFO 49 ("UHD-141056_Smile_WW").
    pub comment: Option<String>,
    /// CINFO 28.
    pub metadata_language: Option<String>,
    /// MSG:1005's last parameter ("MakeMKV v2.0.0 win(x64-release)") and the "2.0.0" inside it (`version_triple` parses it).
    pub makemkv_banner: Option<String>,
    pub makemkv_version: Option<String>,
    /// MSG:3344 seen (6.1 step 4; 8.3 rule 2 trusts labels only then).
    pub java_in_use: bool,
    /// `audio::titles::dvd_scan`, which also holds when CINFO 1 is absent (a DVD log pinned without CINFO).
    pub is_dvd: bool,
    /// Every title that has a TINFO line, by number (SINFO lines of a title without one are not typed).
    pub titles: BTreeMap<u32, Title>,
    /// MSG:2003 lines in the scan (the count; 6.2 step 7 reports them on a rip).
    pub scan_read_errors: u32,
}

impl Disc {
    /// `titles.get(&number)` with the sentence "the scan lists no title <n>" as Err. Spec 9.4.
    pub fn title(&self, number: u32) -> Result<&Title, String> {
        self.titles.get(&number).ok_or_else(|| format!("the scan lists no title {number}"))
    }
}

/// The typed disc of a parsed scan; Err only where an attribute the model needs is malformed (attribute 22 not a number,
/// attribute 19 not WxH), naming the title and the stream. A title exists when the scan has a TINFO line for it; its
/// streams are its SINFO lines in stream order, without a stream whose attribute 1 names no kind. Spec 9.4, 9.5, 6.1
/// step 4.
pub fn disc(scan: &Scan) -> Result<Disc, String> {
    let mut titles = BTreeMap::new();
    for (&number, attrs) in &scan.titles {
        let mut streams = Vec::new();
        for (&(_, stream_number), stream_attrs) in scan.streams.range((number, 0)..=(number, u32::MAX)) {
            if let Some(stream) = typed_stream(number, stream_number, stream_attrs)? {
                streams.push(stream);
            }
        }
        titles.insert(number, typed_title(number, attrs, streams));
    }
    let makemkv_banner =
        scan.messages.iter().find(|message| message.code == 1005).and_then(|message| message.params.last().cloned());
    let makemkv_version = makemkv_banner.as_deref().and_then(version_in_banner);
    let read_errors = scan.messages.iter().filter(|message| message.code == 2003).count();
    Ok(Disc {
        kind: disc_kind(scan.disc.get(&1).map(String::as_str)),
        name: text_attr(&scan.disc, 2),
        tree_name: text_attr(&scan.disc, 30),
        volume_label: text_attr(&scan.disc, 32),
        comment: text_attr(&scan.disc, 49),
        metadata_language: text_attr(&scan.disc, 28),
        makemkv_banner,
        makemkv_version,
        java_in_use: scan.messages.iter().any(|message| message.code == 3344),
        is_dvd: dvd_scan(scan),
        titles,
        scan_read_errors: u32::try_from(read_errors).unwrap_or(u32::MAX),
    })
}

/// CINFO 1's text as MakeMKV writes it: "Blu-ray disc" or "DVD disc"; anything else (or nothing) is Unknown.
fn disc_kind(text: Option<&str>) -> DiscKind {
    match text {
        Some("Blu-ray disc") => DiscKind::BluRay,
        Some("DVD disc") => DiscKind::Dvd,
        _ => DiscKind::Unknown,
    }
}

/// The "N.N.N" word after "MakeMKV v" in the banner, kept only when `version_triple` reads it. Spec 9.1.
fn version_in_banner(banner: &str) -> Option<String> {
    let word = banner.strip_prefix("MakeMKV v")?.split_whitespace().next()?;
    version_triple(word).map(|_| word.to_string())
}

/// Attribute `id` as text; None when it is absent or empty.
fn text_attr(attrs: &Attrs, id: u32) -> Option<String> {
    attrs.get(&id).filter(|value| !value.is_empty()).cloned()
}

/// Attribute `id` as a number (surrounding whitespace ignored); None when it is absent, empty or not a number.
fn number_attr<T: std::str::FromStr>(attrs: &Attrs, id: u32) -> Option<T> {
    attrs.get(&id).and_then(|value| value.trim().parse().ok())
}

/// Attribute 19's "WxH" as (width, height).
fn parse_video_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

/// One stream, typed; None when attribute 1 names no kind (the stream is then left out of its title and not read
/// further). Err for a malformed attribute 22 or a non-empty attribute 19 that is not WxH.
fn typed_stream(title: u32, number: u32, attrs: &Attrs) -> Result<Option<Stream>, String> {
    let Some(kind) = attrs.get(&1).and_then(|text| StreamKind::from_attr(text)) else {
        return Ok(None);
    };
    let flags = Flags::of(attrs).map_err(|why| format!("title {title} stream {number}: attribute 22 (flags): {why}"))?;
    let video_size = match attrs.get(&19).filter(|text| !text.is_empty()) {
        None => None,
        Some(text) => Some(parse_video_size(text).ok_or_else(|| {
            format!("title {title} stream {number}: attribute 19 (video size) is '{text}', not WxH")
        })?),
    };
    let codec_id = attrs.get(&5).cloned().unwrap_or_default();
    let codec_short = attrs.get(&6).cloned().unwrap_or_default();
    let codec_long = attrs.get(&7).cloned().unwrap_or_default();
    let bitrate_text = text_attr(attrs, 13);
    let input = codec(&codec_id, &codec_short, &codec_long);
    let output_short = text_attr(attrs, 41);
    let file_codec = output_short.as_deref().map_or_else(|| input.clone(), output_codec);
    Ok(Some(Stream {
        number,
        kind,
        name: text_attr(attrs, 2),
        language: text_attr(attrs, 3).map(|code| code.to_lowercase()),
        codec: input,
        codec_id,
        codec_short,
        codec_long,
        bitrate_kbps: bitrate_text.as_deref().and_then(bitrate_kbps),
        bitrate_text,
        channels: number_attr(attrs, 14),
        sample_rate: number_attr(attrs, 17),
        sample_size: number_attr(attrs, 18),
        video_size,
        flags,
        tree_info: text_attr(attrs, 30),
        layout_name: text_attr(attrs, 40),
        output_codec: output_short,
        conversion: text_attr(attrs, 42),
        file_codec,
    }))
}

/// One title, typed, with its typed streams.
fn typed_title(number: u32, attrs: &Attrs, streams: Vec<Stream>) -> Title {
    let segment_map = attrs.get(&26).cloned().unwrap_or_default();
    Title {
        number,
        name: text_attr(attrs, 2),
        chapters: number_attr(attrs, 8).unwrap_or(0),
        seconds: attrs.get(&9).and_then(|text| duration_seconds(text)),
        size: number_attr(attrs, 11),
        source_file: text_attr(attrs, 16),
        dvd_title: text_attr(attrs, 24),
        segment_count: number_attr(attrs, 25),
        segments: segments(&segment_map),
        truncated: segment_map.ends_with("..."),
        segment_map,
        output_name: text_attr(attrs, 27),
        comment: text_attr(attrs, 49),
        streams,
    }
}

/// Spec 6.3 step 8: two scans' titles are the same title when `same_source(a.source(), b.source())`, their segment maps
/// agree (`segment_maps_agree`) and their durations are within DURATION_MATCH_S (both durations present); the title
/// numbers are never compared (spec 5). Used by phase 2 for the copy's scan against the disc-time scan;
/// `remembered::match_title` and `lookup::discdb::same_title` follow the same three tests over their own records.
pub fn same_scan_title(a: &Title, b: &Title) -> bool {
    let durations_match = match (a.seconds, b.seconds) {
        (Some(x), Some(y)) => x.abs_diff(y) <= DURATION_MATCH_S,
        _ => false,
    };
    same_source(&a.source(), &b.source()) && segment_maps_agree(&a.segment_map, &b.segment_map) && durations_match
}

/// The `<di:name>` text of `BDMV/META/DL/bdmt_*.xml` (the first `<di:name>` element's text, entities `&amp;`, `&lt;`, `&gt;`,
/// `&quot;`, `&apos;` decoded, surrounding whitespace trimmed, a leading byte-order mark ignored); None when the text has
/// none, when that element is empty or holds only whitespace, or when it is not closed. A start tag with attributes
/// (`<di:name xml:lang="en">`) counts; a longer tag name (`<di:names>`) does not; numeric character references
/// (`&#233;`, `&#xE9;`) are decoded too, and any other entity (`&copy;`) is left as written. Spec 6.1 step 2, 8.1.
pub fn bdmt_name(xml: &str) -> Option<String> {
    const OPEN: &str = "<di:name";
    let mut rest = xml.trim_start_matches('\u{feff}');
    let body = loop {
        let at = rest.find(OPEN)?;
        let after = &rest[at + OPEN.len()..];
        match after.chars().next() {
            Some('>') => break &after[1..],
            Some(c) if c == '/' || c.is_whitespace() => {
                let close = after.find('>')?;
                if after[..close].ends_with('/') {
                    return None;
                }
                break &after[close + 1..];
            }
            _ => rest = after,
        }
    };
    let end = body.find("</di:name>")?;
    let decoded = decode_entities(&body[..end]);
    let name = decoded.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// The numeric character references and the five XML entities of `bdmt_name` decoded, each once, left to right; any
/// other `&` is kept.
fn decode_entities(text: &str) -> String {
    const ENTITIES: [(&str, char); 5] = [("&amp;", '&'), ("&lt;", '<'), ("&gt;", '>'), ("&quot;", '"'), ("&apos;", '\'')];
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some((character, length)) = character_reference(tail) {
            out.push(character);
            rest = &tail[length..];
            continue;
        }
        match ENTITIES.iter().find(|(entity, _)| tail.starts_with(*entity)) {
            Some((entity, character)) => {
                out.push(*character);
                rest = &tail[entity.len()..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The character a numeric character reference at the start of `tail` names (`&#` and decimal digits, or `&#x` and
/// hexadecimal digits, then `;`) and the reference's length in bytes; None when `tail` does not start with one, or when
/// `char::from_u32` refuses its value (a surrogate, above U+10FFFF, or too many digits for a u32).
fn character_reference(tail: &str) -> Option<(char, usize)> {
    let body = tail.strip_prefix("&#")?;
    let (marker, radix) = if body.starts_with('x') { (1, 16) } else { (0, 10) };
    let digits_on = &body[marker..];
    let end = digits_on.find(';')?;
    let digits = &digits_on[..end];
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let character = char::from_u32(u32::from_str_radix(digits, radix).ok()?)?;
    Some((character, "&#".len() + marker + end + 1))
}

/// The bdmt file names of a recording's nav tree, English first ("bdmt_eng.xml", then the others in name order), so the
/// caller reads the first: the names matching `bdmt_*.xml` ignoring ASCII case, without duplicates, `bdmt_eng.xml` (any
/// case) first, the rest in byte order. Spec 6.1 step 2.
pub fn bdmt_candidates(names: &[String]) -> Vec<String> {
    let mut found: Vec<String> = names
        .iter()
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("bdmt_") && lower.ends_with(".xml")
        })
        .cloned()
        .collect();
    found.sort();
    found.dedup();
    if let Some(at) = found.iter().position(|name| name.eq_ignore_ascii_case("bdmt_eng.xml")) {
        let english = found.remove(at);
        found.insert(0, english);
    }
    found
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::codec::{codec, Family};
    use crate::robot::parse;
    use crate::testsupport::{scan_text, scan_text_with_messages};

    /// Attributes from (id, value) pairs, as the parser stores them.
    fn attrs(pairs: &[(u32, &str)]) -> Attrs {
        pairs.iter().map(|&(id, value)| (id, value.to_string())).collect()
    }

    #[test]
    fn flags_name_the_bits_of_spec_9_5() {
        assert_eq!(
            [
                Flags::DIRECTORS_COMMENTS,
                Flags::ALT_DIRECTORS_COMMENTS,
                Flags::VISUALLY_IMPAIRED,
                Flags::CORE_AUDIO,
                Flags::SECONDARY_AUDIO,
                Flags::HAS_CORE_AUDIO,
                Flags::DERIVED,
                Flags::FORCED,
            ],
            [1, 2, 4, 256, 512, 1024, 2048, 4096]
        );
        assert_eq!(Flags::KNOWN, 7943);
        assert_eq!(Flags::default(), Flags(0));
    }

    #[test]
    fn the_derived_bit_is_the_audio_labels_derived_bit() {
        assert_eq!(Flags::DERIVED as i64, crate::audio::labels::DERIVED);
    }

    #[test]
    fn flags_of_reads_attribute_22_and_is_zero_when_it_is_absent_or_empty() {
        assert_eq!(Flags::of(&attrs(&[(22, "2304")])), Ok(Flags(2304)));
        assert_eq!(Flags::of(&attrs(&[(22, "65536")])), Ok(Flags(65536)));
        assert_eq!(Flags::of(&attrs(&[(22, "4294967295")])), Ok(Flags(u32::MAX)));
        assert_eq!(Flags::of(&attrs(&[(22, "")])), Ok(Flags(0)));
        assert_eq!(Flags::of(&attrs(&[(1, "Audio")])), Ok(Flags(0)));
    }

    #[test]
    fn flags_of_refuses_text_and_numbers_outside_the_u32_range() {
        assert_eq!(
            Flags::of(&attrs(&[(22, "x")])),
            Err("invalid literal for int() with base 10: 'x'".to_string())
        );
        assert_eq!(
            Flags::of(&attrs(&[(22, "-1")])),
            Err("attribute 22 is -1, outside the flag range 0 to 4294967295".to_string())
        );
        assert_eq!(
            Flags::of(&attrs(&[(22, "4294967296")])),
            Err("attribute 22 is 4294967296, outside the flag range 0 to 4294967295".to_string())
        );
    }

    #[test]
    fn unknown_bits_are_kept_in_the_value_and_ignored_by_every_query() {
        let unknown = Flags(65536);
        assert_eq!(unknown.0, 65536);
        assert_eq!(unknown.known(), 0);
        assert!(!unknown.has(65536));
        assert!(!unknown.commentary());
        assert!(!unknown.derived_forced_only());
        assert!(!unknown.forced_authored());
        let mixed = Flags(16384 | 32768 | 65536 | 4096);
        assert_eq!(mixed.known(), 4096);
        assert!(mixed.has(Flags::FORCED));
        assert!(mixed.forced_authored());
        assert!(!mixed.has(Flags::FORCED | 65536));
    }

    #[test]
    fn has_needs_every_known_bit_of_its_argument_and_never_holds_for_zero() {
        assert!(Flags(6144).has(Flags::FORCED | Flags::DERIVED));
        assert!(Flags(6144).has(Flags::FORCED));
        assert!(!Flags(4096).has(Flags::FORCED | Flags::DERIVED));
        assert!(!Flags(6144).has(0));
        assert!(!Flags(0).has(0));
        assert!(Flags(2304).has(Flags::CORE_AUDIO) && Flags(2304).has(Flags::DERIVED));
        assert!(Flags(1024).has(Flags::HAS_CORE_AUDIO) && !Flags(1024).has(Flags::DERIVED));
        assert!(Flags(512).has(Flags::SECONDARY_AUDIO));
    }

    #[test]
    fn commentary_holds_for_flags_1_2_and_4_only() {
        for bits in [1, 2, 4, 1 | 4, 1 | 2304] {
            assert!(Flags(bits).commentary(), "{bits}");
        }
        for bits in [0, 256, 512, 1024, 2048, 2304, 4096, 6144, 65536] {
            assert!(!Flags(bits).commentary(), "{bits}");
        }
    }

    #[test]
    fn derived_forced_only_is_4096_with_2048_and_forced_authored_is_4096_without_it() {
        assert!(Flags(6144).derived_forced_only() && !Flags(6144).forced_authored());
        assert!(Flags(4096).forced_authored() && !Flags(4096).derived_forced_only());
        assert!(!Flags(2048).derived_forced_only() && !Flags(2048).forced_authored());
        assert!(!Flags(0).derived_forced_only() && !Flags(0).forced_authored());
    }

    #[test]
    fn flags_serialise_as_the_plain_number() {
        assert_eq!(serde_json::to_string(&Flags(6144)).unwrap(), "6144");
        assert_eq!(serde_json::from_str::<Flags>("65536").unwrap(), Flags(65536));
    }

    #[test]
    fn stream_kinds_read_attribute_1_as_makemkv_spells_it() {
        assert_eq!(StreamKind::from_attr("Video"), Some(StreamKind::Video));
        assert_eq!(StreamKind::from_attr("Audio"), Some(StreamKind::Audio));
        assert_eq!(StreamKind::from_attr("Subtitles"), Some(StreamKind::Subtitles));
        for other in ["video", "Subtitle", "", "Data"] {
            assert_eq!(StreamKind::from_attr(other), None, "{other:?}");
        }
    }

    #[test]
    fn stream_kinds_spell_as_serde_does() {
        let kinds = [StreamKind::Video, StreamKind::Audio, StreamKind::Subtitles];
        assert_eq!(kinds.map(StreamKind::as_str), ["video", "audio", "subtitles"]);
        for kind in kinds {
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{}\"", kind.as_str()));
            assert_eq!(serde_json::from_str::<StreamKind>(&format!("\"{}\"", kind.as_str())).unwrap(), kind);
        }
    }

    #[test]
    fn a_title_source_is_the_source_file_first_then_the_dvd_title_number() {
        assert_eq!(
            title_source(Some("00800.mpls"), Some("01")),
            TitleSource::File { name: "00800.mpls".to_string() }
        );
        assert_eq!(title_source(Some(""), Some("01")), TitleSource::DvdTitle { number: 1 });
        assert_eq!(title_source(None, Some("01")), TitleSource::DvdTitle { number: 1 });
        assert_eq!(title_source(None, Some(" 11 ")), TitleSource::DvdTitle { number: 11 });
        assert_eq!(title_source(None, Some("A1")), TitleSource::Unknown);
        assert_eq!(title_source(None, Some("")), TitleSource::Unknown);
        assert_eq!(title_source(None, None), TitleSource::Unknown);
    }

    #[test]
    fn same_source_compares_dvd_title_numbers_as_numbers() {
        assert!(same_source(&title_source(None, Some("01")), &title_source(None, Some("1"))));
        assert!(same_source(&title_source(None, Some("11")), &title_source(None, Some("011"))));
        assert!(!same_source(&title_source(None, Some("01")), &title_source(None, Some("03"))));
    }

    #[test]
    fn same_source_compares_source_files_exactly_and_never_matches_unknown() {
        let playlist = title_source(Some("00001.mpls"), None);
        assert!(same_source(&playlist, &playlist.clone()));
        assert!(!same_source(&playlist, &title_source(Some("00001.m2ts"), None)));
        assert!(!same_source(&title_source(Some("1"), None), &title_source(None, Some("1"))));
        assert!(!same_source(&TitleSource::Unknown, &TitleSource::Unknown));
    }

    #[test]
    fn title_sources_serialise_tagged_by_how_the_title_is_known() {
        let cases = [
            (TitleSource::File { name: "00800.mpls".to_string() }, r#"{"by":"file","name":"00800.mpls"}"#),
            (TitleSource::DvdTitle { number: 3 }, r#"{"by":"dvd_title","number":3}"#),
            (TitleSource::Unknown, r#"{"by":"unknown"}"#),
        ];
        for (source, json) in cases {
            assert_eq!(serde_json::to_string(&source).unwrap(), json);
            assert_eq!(serde_json::from_str::<TitleSource>(json).unwrap(), source);
        }
    }

    #[test]
    fn the_duration_match_is_the_reconcilers_tolerance() {
        assert_eq!(DURATION_MATCH_S, 2);
        assert_eq!(DURATION_MATCH_S as i64, crate::reconcile::DURATION_TOLERANCE_S);
    }

    #[test]
    fn untruncated_segment_maps_agree_only_when_equal() {
        assert!(segment_maps_agree("1-19", "1-19"));
        assert!(segment_maps_agree("", ""));
        assert!(segment_maps_agree("280,284,299", "280,284,299"));
        assert!(!segment_maps_agree("1,2", "1,2,3"));
        assert!(!segment_maps_agree("1-5,6-9,10-14,15", "1-5,6-10,11-14,15"));
        assert!(!segment_maps_agree("6", ""));
    }

    #[test]
    fn a_truncated_map_agrees_when_the_other_starts_with_its_known_prefix() {
        let full = "27,28,29,30,31,32";
        assert!(segment_maps_agree("27,28,29,...", full));
        assert!(segment_maps_agree(full, "27,28,29,..."));
        assert!(segment_maps_agree("27,28,29,30,31,32,...", full));
        assert!(!segment_maps_agree("27,28,...", "27,29,30"));
        assert!(!segment_maps_agree("27,29,30", "27,28,..."));
        assert!(!segment_maps_agree("27,28,29,30,31,32,33,...", full));
    }

    #[test]
    fn a_partial_number_before_the_dots_is_not_compared() {
        assert!(segment_maps_agree("10,11,12...", "10,11,125,126"));
        assert!(segment_maps_agree("10,11,12...", "10,11"));
        assert!(!segment_maps_agree("10,11,12...", "10,12,13"));
        // Parts are compared whole, never as text: "28" does not start "281".
        assert!(!segment_maps_agree("281,...", "28,1"));
        assert!(!segment_maps_agree("27,28,...", "27,281,30"));
    }

    #[test]
    fn two_truncated_maps_agree_when_the_shorter_known_prefix_starts_the_longer() {
        assert!(segment_maps_agree("1,2,3,...", "1,2,..."));
        assert!(segment_maps_agree("1,2,...", "1,2,3,4,5..."));
        assert!(!segment_maps_agree("1,2,3,...", "1,4,..."));
    }

    #[test]
    fn a_map_cut_before_its_first_whole_part_agrees_with_any_map() {
        assert!(segment_maps_agree("...", "5,6"));
        assert!(segment_maps_agree("12...", "7"));
        assert!(segment_maps_agree("...", ""));
    }

    /// A Blu-ray scan with one title of every attribute the model reads: video, a DTS-HD MA track and its derived DTS
    /// core, and a forced-only PGS stream (stream 3 is absent: the numbering is sparse). The values are the recorded ones
    /// of pans-labyrinth-2006-uhd's title 0, except a made-up disc comment (CINFO 49), title comment (attribute 49) and
    /// output name (attribute 27), an upper-cased `SPA`, the DTS core (stream 2) without its attributes 30 and 42, and
    /// the recorded forced-only PGS stream 5 numbered 4 (the recorded streams 3 and 4 are left out).
    fn full_scan() -> String {
        scan_text(
            &[
                (1, "Blu-ray disc"),
                (2, "Pan's Labyrinth"),
                (28, "eng"),
                (30, "Pan's Labyrinth"),
                (32, "PANS_LABYRINTH"),
                (49, "Retail"),
            ],
            &[(
                0,
                &[
                    (2, "Pan's Labyrinth"),
                    (8, "22"),
                    (9, "1:59:10"),
                    (11, "55613952000"),
                    (16, "00800.mpls"),
                    (25, "1"),
                    (26, "6"),
                    (27, "title_t00.mkv"),
                    (49, "FPL_MainFeature"),
                ],
                &[
                    (
                        0,
                        &[
                            (1, "Video"),
                            (5, "V_MPEGH/ISO/HEVC"),
                            (6, "MpegH"),
                            (7, "MpegH HEVC Main10@L5.1"),
                            (19, "3840x2160"),
                            (22, "0"),
                            (30, "MpegH HEVC Main10@L5.1"),
                            (42, "( Lossless conversion )"),
                        ],
                    ),
                    (
                        1,
                        &[
                            (1, "Audio"),
                            (2, "Surround 5.1"),
                            (3, "SPA"),
                            (5, "A_DTS"),
                            (6, "DTS-HD MA"),
                            (7, "DTS-HD Master Audio"),
                            (14, "6"),
                            (17, "48000"),
                            (18, "24"),
                            (22, "1024"),
                            (30, "DTS-HD MA Surround 5.1 Spanish"),
                            (40, "5.1(side)"),
                            (42, "( Lossless conversion )"),
                        ],
                    ),
                    (
                        2,
                        &[
                            (1, "Audio"),
                            (2, "Surround 5.1"),
                            (3, "spa"),
                            (5, "A_DTS"),
                            (6, "DTS"),
                            (7, "DTS"),
                            (13, "1.5 Mb/s"),
                            (14, "6"),
                            (17, "48000"),
                            (22, "2304"),
                            (40, "5.1(side)"),
                        ],
                    ),
                    (
                        4,
                        &[
                            (1, "Subtitles"),
                            (3, "eng"),
                            (5, "S_HDMV/PGS"),
                            (6, "PGS"),
                            (7, "HDMV PGS Subtitles"),
                            (22, "6144"),
                            (30, "PGS English  (forced only)"),
                            (42, "( Lossless conversion )"),
                        ],
                    ),
                ],
            )],
        )
    }

    #[test]
    fn disc_types_every_attribute_of_a_title_and_its_streams() {
        let typed = disc(&parse(&full_scan())).unwrap();
        assert_eq!(typed.kind, DiscKind::BluRay);
        assert_eq!(typed.name.as_deref(), Some("Pan's Labyrinth"));
        assert_eq!(typed.tree_name.as_deref(), Some("Pan's Labyrinth"));
        assert_eq!(typed.volume_label.as_deref(), Some("PANS_LABYRINTH"));
        assert_eq!(typed.comment.as_deref(), Some("Retail"));
        assert_eq!(typed.metadata_language.as_deref(), Some("eng"));
        assert!(!typed.is_dvd);
        let title = typed.title(0).unwrap();
        assert_eq!(title.number, 0);
        assert_eq!(title.name.as_deref(), Some("Pan's Labyrinth"));
        assert_eq!((title.chapters, title.seconds, title.size), (22, Some(7150), Some(55_613_952_000)));
        assert_eq!(title.source_file.as_deref(), Some("00800.mpls"));
        assert_eq!(title.dvd_title, None);
        assert_eq!(title.segment_count, Some(1));
        assert_eq!((title.segment_map.as_str(), title.truncated), ("6", false));
        assert_eq!(title.segments, [6]);
        assert_eq!(title.output_name.as_deref(), Some("title_t00.mkv"));
        assert_eq!(title.comment.as_deref(), Some("FPL_MainFeature"));
        let numbers: Vec<u32> = title.streams.iter().map(|stream| stream.number).collect();
        assert_eq!(numbers, [0, 1, 2, 4]);
        assert_eq!(
            title.streams[0],
            Stream {
                number: 0,
                kind: StreamKind::Video,
                name: None,
                language: None,
                codec: codec("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1"),
                codec_id: "V_MPEGH/ISO/HEVC".to_string(),
                codec_short: "MpegH".to_string(),
                codec_long: "MpegH HEVC Main10@L5.1".to_string(),
                bitrate_text: None,
                bitrate_kbps: None,
                channels: None,
                sample_rate: None,
                sample_size: None,
                video_size: Some((3840, 2160)),
                flags: Flags(0),
                tree_info: Some("MpegH HEVC Main10@L5.1".to_string()),
                layout_name: None,
                output_codec: None,
                conversion: Some("( Lossless conversion )".to_string()),
                file_codec: codec("V_MPEGH/ISO/HEVC", "MpegH", "MpegH HEVC Main10@L5.1"),
            }
        );
        assert_eq!(
            title.streams[1],
            Stream {
                number: 1,
                kind: StreamKind::Audio,
                name: Some("Surround 5.1".to_string()),
                language: Some("spa".to_string()),
                codec: codec("A_DTS", "DTS-HD MA", "DTS-HD Master Audio"),
                codec_id: "A_DTS".to_string(),
                codec_short: "DTS-HD MA".to_string(),
                codec_long: "DTS-HD Master Audio".to_string(),
                bitrate_text: None,
                bitrate_kbps: None,
                channels: Some(6),
                sample_rate: Some(48000),
                sample_size: Some(24),
                video_size: None,
                flags: Flags(1024),
                tree_info: Some("DTS-HD MA Surround 5.1 Spanish".to_string()),
                layout_name: Some("5.1(side)".to_string()),
                output_codec: None,
                conversion: Some("( Lossless conversion )".to_string()),
                file_codec: codec("A_DTS", "DTS-HD MA", "DTS-HD Master Audio"),
            }
        );
        let core = &title.streams[2];
        assert_eq!((core.bitrate_text.as_deref(), core.bitrate_kbps), (Some("1.5 Mb/s"), Some(1500)));
        assert_eq!((core.flags, core.tree_info.clone(), core.conversion.clone()), (Flags(2304), None, None));
        let forced = &title.streams[3];
        assert_eq!((forced.number, forced.kind, forced.language.as_deref()), (4, StreamKind::Subtitles, Some("eng")));
        assert_eq!((forced.flags, forced.tree_info.as_deref()), (Flags(6144), Some("PGS English  (forced only)")));
        let families: Vec<Family> = title.streams.iter().map(|stream| stream.codec.family).collect();
        assert_eq!(families, [Family::Hevc, Family::DtsHdMa, Family::Dts, Family::Pgs]);
    }

    #[test]
    fn messages_give_the_makemkv_banner_version_java_use_and_read_error_count() {
        let mut text = scan_text_with_messages("2.0.0", true, &[(1, "DVD disc")], &[(0, &[(9, "1:50:06")], &[])]);
        for _ in 0..2 {
            text.push_str(concat!(
                "MSG:2003,0,3,\"Error 'x' occurred while reading '/VIDEO_TS/VTS_01_1.VOB' at offset '1048576'\",",
                "\"Error '%1' occurred while reading '%2' at offset '%3'\",\"x\",\"/VIDEO_TS/VTS_01_1.VOB\",\"1048576\"\n",
            ));
        }
        let typed = disc(&parse(&text)).unwrap();
        assert_eq!(typed.makemkv_banner.as_deref(), Some("MakeMKV v2.0.0 win(x64-release)"));
        assert_eq!(typed.makemkv_version.as_deref(), Some("2.0.0"));
        assert!(typed.java_in_use);
        assert_eq!(typed.scan_read_errors, 2);
        let without = disc(&parse(&scan_text_with_messages("1.18.4", false, &[(1, "Blu-ray disc")], &[]))).unwrap();
        assert_eq!(without.makemkv_version.as_deref(), Some("1.18.4"));
        assert!(!without.java_in_use);
        assert_eq!(without.scan_read_errors, 0);
        let silent = disc(&parse(&scan_text(&[(1, "Blu-ray disc")], &[]))).unwrap();
        assert_eq!((silent.makemkv_banner, silent.makemkv_version, silent.java_in_use), (None, None, false));
    }

    #[test]
    fn a_banner_whose_version_is_not_n_n_n_keeps_the_banner_without_a_version() {
        let beta = disc(&parse("MSG:1005,0,1,\"MakeMKV v2.0 beta started\",\"%1 started\",\"MakeMKV v2.0 beta\"\n")).unwrap();
        assert_eq!((beta.makemkv_banner.as_deref(), beta.makemkv_version.as_deref()), (Some("MakeMKV v2.0 beta"), None));
        let other = disc(&parse("MSG:1005,0,1,\"started\",\"%1 started\",\"lazymkv test\"\n")).unwrap();
        assert_eq!((other.makemkv_banner.as_deref(), other.makemkv_version.as_deref()), (Some("lazymkv test"), None));
        let bare = disc(&parse("MSG:1005,0,0,\"MakeMKV v2.0.0 started\",\"%1 started\"\n")).unwrap();
        assert_eq!((bare.makemkv_banner, bare.makemkv_version), (None, None));
        let twice = disc(&parse(concat!(
            "MSG:1005,0,1,\"a\",\"%1\",\"MakeMKV v1.18.4 win(x64-release)\"\n",
            "MSG:1005,0,1,\"b\",\"%1\",\"MakeMKV v2.0.0 win(x64-release)\"\n",
        )))
        .unwrap();
        assert_eq!(twice.makemkv_version.as_deref(), Some("1.18.4"));
    }

    #[test]
    fn absent_empty_and_unreadable_attributes_read_as_none_and_chapters_as_zero() {
        let text = scan_text(
            &[],
            &[
                (
                    0,
                    &[(8, "x"), (9, "1:41"), (11, "big"), (16, ""), (25, "")],
                    &[(0, &[(1, "Audio"), (3, ""), (13, ""), (14, "six")]), (1, &[(1, "Subtitles"), (3, "UND")])],
                ),
                (1, &[(9, "0:00:20")], &[]),
            ],
        );
        let typed = disc(&parse(&text)).unwrap();
        let odd = typed.title(0).unwrap();
        assert_eq!((odd.chapters, odd.seconds, odd.size), (0, None, None));
        assert_eq!((odd.name.clone(), odd.source_file.clone(), odd.dvd_title.clone()), (None, None, None));
        assert_eq!(odd.segment_count, None);
        assert_eq!((odd.segment_map.as_str(), odd.segments.is_empty(), odd.truncated), ("", true, false));
        assert_eq!((odd.output_name.clone(), odd.comment.clone()), (None, None));
        assert_eq!(odd.source(), TitleSource::Unknown);
        let stream = &odd.streams[0];
        assert_eq!((stream.name.clone(), stream.language.clone(), stream.channels), (None, None, None));
        assert_eq!((stream.bitrate_text.clone(), stream.bitrate_kbps), (None, None));
        assert_eq!((stream.codec_id.as_str(), stream.codec_short.as_str(), stream.codec_long.as_str()), ("", "", ""));
        assert_eq!(stream.codec, codec("", "", ""));
        assert_eq!(stream.flags, Flags(0));
        assert_eq!((stream.sample_rate, stream.sample_size, stream.video_size), (None, None, None));
        assert_eq!(
            (stream.tree_info.clone(), stream.layout_name.clone(), stream.output_codec.clone(), stream.conversion.clone()),
            (None, None, None, None)
        );
        assert_eq!(odd.streams[1].language.as_deref(), Some("und"));
        let short = typed.title(1).unwrap();
        assert_eq!((short.seconds, short.chapters, short.streams.len()), (Some(20), 0, 0));
    }

    #[test]
    fn a_stream_whose_attribute_1_names_no_kind_is_left_out_and_the_others_keep_their_numbers() {
        let text = scan_text(
            &[],
            &[(
                0,
                &[(9, "0:10:00")],
                &[(0, &[(1, "Video")]), (1, &[(1, "Data")]), (2, &[(3, "eng")]), (3, &[(1, "Audio"), (3, "eng")])],
            )],
        );
        let typed = disc(&parse(&text)).unwrap();
        let kept: Vec<(u32, StreamKind)> =
            typed.title(0).unwrap().streams.iter().map(|stream| (stream.number, stream.kind)).collect();
        assert_eq!(kept, [(0, StreamKind::Video), (3, StreamKind::Audio)]);
    }

    #[test]
    fn streams_of_a_title_without_tinfo_lines_are_not_typed() {
        let text = scan_text(&[], &[(0, &[(9, "0:10:00")], &[(0, &[(1, "Video")])]), (5, &[], &[(0, &[(1, "Video")])])]);
        let scan = parse(&text);
        assert!(scan.streams.contains_key(&(5, 0)));
        let typed = disc(&scan).unwrap();
        assert_eq!(typed.titles.keys().copied().collect::<Vec<u32>>(), [0]);
    }

    #[test]
    fn a_malformed_flag_or_video_size_refuses_the_scan_naming_the_title_and_stream() {
        let with = |stream: &[(u32, &str)]| parse(&scan_text(&[], &[(2, &[(9, "0:01:00")], &[(3, stream)])]));
        assert_eq!(
            disc(&with(&[(1, "Audio"), (22, "lots")])),
            Err("title 2 stream 3: attribute 22 (flags): invalid literal for int() with base 10: 'lots'".to_string())
        );
        assert_eq!(
            disc(&with(&[(1, "Audio"), (22, "-4")])),
            Err("title 2 stream 3: attribute 22 (flags): attribute 22 is -4, outside the flag range 0 to 4294967295"
                .to_string())
        );
        assert_eq!(
            disc(&with(&[(1, "Video"), (19, "1920 by 1080")])),
            Err("title 2 stream 3: attribute 19 (video size) is '1920 by 1080', not WxH".to_string())
        );
        assert_eq!(
            disc(&with(&[(1, "Video"), (19, "1920x")])),
            Err("title 2 stream 3: attribute 19 (video size) is '1920x', not WxH".to_string())
        );
        let empty_size = disc(&with(&[(1, "Video"), (19, "")])).unwrap();
        assert_eq!(empty_size.title(2).unwrap().streams[0].video_size, None);
        // A stream left out for its attribute 1 is not read further.
        assert!(disc(&with(&[(1, "Data"), (22, "lots")])).is_ok());
    }

    #[test]
    fn the_disc_kind_is_read_from_cinfo_1_and_spelt_as_serde_does() {
        let kind_of = |cinfo: &[(u32, &str)]| disc(&parse(&scan_text(cinfo, &[]))).unwrap().kind;
        assert_eq!(kind_of(&[(1, "Blu-ray disc")]), DiscKind::BluRay);
        assert_eq!(kind_of(&[(1, "DVD disc")]), DiscKind::Dvd);
        assert_eq!(kind_of(&[(1, "HD-DVD disc")]), DiscKind::Unknown);
        assert_eq!(kind_of(&[(2, "No kind")]), DiscKind::Unknown);
        for (kind, spelt) in [(DiscKind::BluRay, "blu-ray"), (DiscKind::Dvd, "dvd"), (DiscKind::Unknown, "unknown")] {
            assert_eq!(kind.as_str(), spelt);
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{spelt}\""));
        }
    }

    #[test]
    fn a_dvd_is_detected_as_dvd_scan_detects_it_even_without_cinfo() {
        let dvd_video = scan_text(&[], &[(0, &[(9, "1:38:24"), (24, "01")], &[(0, &[(1, "Video"), (5, "V_MPEG2")])])]);
        let typed = disc(&parse(&dvd_video)).unwrap();
        assert!(typed.is_dvd);
        assert_eq!(typed.kind, DiscKind::Unknown);
        let with_playlist = scan_text(
            &[(1, "DVD disc")],
            &[(0, &[(9, "1:38:24"), (16, "00001.mpls")], &[(0, &[(1, "Video"), (5, "V_MPEG2")])])],
        );
        assert!(!disc(&parse(&with_playlist)).unwrap().is_dvd);
        let blu_ray =
            scan_text(&[(1, "Blu-ray disc")], &[(0, &[(9, "1:38:24")], &[(0, &[(1, "Video"), (5, "V_MPEG4/ISO/AVC")])])]);
        assert!(!disc(&parse(&blu_ray)).unwrap().is_dvd);
        assert!(disc(&parse(&scan_text(&[(1, "DVD disc")], &[]))).unwrap().is_dvd);
    }

    #[test]
    fn disc_title_names_a_title_the_scan_lacks() {
        let typed = disc(&parse(&full_scan())).unwrap();
        assert_eq!(typed.title(7), Err("the scan lists no title 7".to_string()));
        assert!(typed.title(0).is_ok());
    }

    #[test]
    fn a_title_gives_its_streams_of_a_kind_its_video_height_and_its_minutes() {
        let typed = disc(&parse(&full_scan())).unwrap();
        let title = typed.title(0).unwrap();
        let numbers = |kind| title.streams_of(kind).iter().map(|stream| stream.number).collect::<Vec<u32>>();
        assert_eq!(numbers(StreamKind::Video), [0]);
        assert_eq!(numbers(StreamKind::Audio), [1, 2]);
        assert_eq!(numbers(StreamKind::Subtitles), [4]);
        assert_eq!(title.video_height(), Some(2160));
        assert_eq!(title.minutes(), Some(7150.0 / 60.0));
        let text = scan_text(&[], &[(0, &[(8, "1")], &[(0, &[(1, "Video")]), (1, &[(1, "Video"), (19, "1920x1080")])])]);
        let bare = disc(&parse(&text)).unwrap();
        let untimed = bare.title(0).unwrap();
        assert_eq!(untimed.minutes(), None);
        // The first video stream has no attribute 19; a later one's size does not stand in for it.
        assert_eq!(untimed.video_height(), None);
        assert!(untimed.streams_of(StreamKind::Audio).is_empty());
    }

    #[test]
    fn the_signature_set_holds_each_audio_and_subtitle_stream_once_by_kind_language_codec_and_channels() {
        let typed = disc(&parse(&full_scan())).unwrap();
        let expected: BTreeSet<(StreamKind, Option<String>, Family, Option<u32>)> = [
            (StreamKind::Audio, Some("spa".to_string()), Family::DtsHdMa, Some(6)),
            (StreamKind::Audio, Some("spa".to_string()), Family::Dts, Some(6)),
            (StreamKind::Subtitles, Some("eng".to_string()), Family::Pgs, None),
        ]
        .into_iter()
        .collect();
        assert_eq!(typed.title(0).unwrap().signature_set(), expected);
        let twins = scan_text(
            &[],
            &[(
                0,
                &[(9, "0:21:18")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG4/ISO/AVC")]),
                    (1, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")]),
                    (2, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")]),
                ],
            )],
        );
        let both = disc(&parse(&twins)).unwrap();
        let set: Vec<(StreamKind, Option<String>, Family, Option<u32>)> =
            both.title(0).unwrap().signature_set().into_iter().collect();
        assert_eq!(set, [(StreamKind::Audio, Some("eng".to_string()), Family::Ac3, Some(2))]);
    }

    #[test]
    fn a_truncated_segment_map_is_marked_and_its_whole_numbers_kept() {
        let text = scan_text(
            &[],
            &[(0, &[(9, "0:05:40"), (26, "27,28,29,3...")], &[]), (1, &[(9, "1:41:13"), (26, "1-3,7")], &[])],
        );
        let typed = disc(&parse(&text)).unwrap();
        let cut = typed.title(0).unwrap();
        assert_eq!((cut.segment_map.as_str(), cut.truncated), ("27,28,29,3...", true));
        assert_eq!(cut.segments, [27, 28, 29]);
        let whole = typed.title(1).unwrap();
        assert_eq!((whole.segment_map.as_str(), whole.truncated), ("1-3,7", false));
        assert_eq!(whole.segments, [1, 2, 3, 7]);
    }

    #[test]
    fn a_title_is_known_by_its_source_file_or_its_dvd_title_number() {
        let typed = disc(&parse(&full_scan())).unwrap();
        assert_eq!(typed.title(0).unwrap().source(), TitleSource::File { name: "00800.mpls".to_string() });
        let dvd = disc(&parse(&scan_text(&[(1, "DVD disc")], &[(1, &[(9, "0:02:20"), (24, "03")], &[])]))).unwrap();
        assert_eq!(dvd.title(1).unwrap().source(), TitleSource::DvdTitle { number: 3 });
    }

    #[test]
    fn a_disc_round_trips_through_json_in_field_order() {
        let text = format!("{}{}", scan_text_with_messages("2.0.0", true, &[], &[]), full_scan());
        let typed = disc(&parse(&text)).unwrap();
        let json = serde_json::to_string(&typed).unwrap();
        assert!(
            json.starts_with(concat!(
                r#"{"kind":"blu-ray","name":"Pan's Labyrinth","tree_name":"Pan's Labyrinth","#,
                r#""volume_label":"PANS_LABYRINTH","comment":"Retail","metadata_language":"eng","#,
                r#""makemkv_banner":"MakeMKV v2.0.0 win(x64-release)","makemkv_version":"2.0.0","#,
                r#""java_in_use":true,"is_dvd":false,"titles":{"0":{"number":0,"name":"Pan's Labyrinth","#,
            )),
            "{json}"
        );
        assert!(json.ends_with(r#""scan_read_errors":0}"#), "{json}");
        let back: Disc = serde_json::from_str(&json).unwrap();
        assert_eq!(back, typed);
    }

    /// Title `number` of a one-title Blu-ray scan with this source file, segment map and duration.
    fn feature(number: u32, source: &str, map: &str, duration: &str) -> Title {
        let text = scan_text(&[(1, "Blu-ray disc")], &[(number, &[(9, duration), (16, source), (26, map)], &[])]);
        let typed = disc(&parse(&text)).unwrap();
        let title = typed.title(number).unwrap().clone();
        title
    }

    /// Title `number` of a one-title DVD scan with this attribute 24 (None writes none), segment map "1-2" and 0:02:20.
    fn dvd_title(number: u32, attribute_24: Option<&str>) -> Title {
        let attrs: Vec<(u32, &str)> = match attribute_24 {
            Some(text) => vec![(9, "0:02:20"), (24, text), (26, "1-2")],
            None => vec![(9, "0:02:20"), (26, "1-2")],
        };
        let text = scan_text(&[(1, "DVD disc")], &[(number, &attrs, &[])]);
        let typed = disc(&parse(&text)).unwrap();
        let title = typed.title(number).unwrap().clone();
        title
    }

    #[test]
    fn a_copys_title_is_the_same_title_by_source_segment_map_and_duration_whatever_its_number() {
        let at_disc_time = feature(3, "00800.mpls", "6", "1:59:10");
        assert!(same_scan_title(&at_disc_time, &feature(0, "00800.mpls", "6", "1:59:10")));
        assert!(same_scan_title(&at_disc_time, &feature(0, "00800.mpls", "6", "1:59:11")));
        assert!(same_scan_title(&at_disc_time, &feature(0, "00800.mpls", "6", "1:59:12")));
        assert!(same_scan_title(&at_disc_time, &feature(0, "00800.mpls", "6", "1:59:08")));
        assert!(!same_scan_title(&at_disc_time, &feature(0, "00800.mpls", "6", "1:59:13")));
        assert!(!same_scan_title(&at_disc_time, &feature(0, "00800.mpls", "6", "1:59:07")));
    }

    #[test]
    fn same_scan_title_needs_the_same_source_file_and_agreeing_segment_maps() {
        let at_disc_time = feature(3, "00055.mpls", "27,28,29,30", "0:05:40");
        assert!(!same_scan_title(&at_disc_time, &feature(3, "00056.mpls", "27,28,29,30", "0:05:40")));
        assert!(!same_scan_title(&at_disc_time, &feature(3, "00055.mpls", "27,28,29,31", "0:05:40")));
        assert!(same_scan_title(&at_disc_time, &feature(1, "00055.mpls", "27,28,2...", "0:05:40")));
        assert!(!same_scan_title(&at_disc_time, &feature(1, "00055.mpls", "27,29,...", "0:05:40")));
    }

    #[test]
    fn same_scan_title_never_matches_without_both_durations() {
        let timed = feature(0, "00800.mpls", "6", "1:59:10");
        let untimed = feature(0, "00800.mpls", "6", "1:59");
        assert_eq!(untimed.seconds, None);
        assert!(!same_scan_title(&timed, &untimed));
        assert!(!same_scan_title(&untimed, &timed));
        assert!(!same_scan_title(&untimed, &untimed));
    }

    #[test]
    fn on_a_dvd_scan_titles_match_by_attribute_24_as_a_number() {
        let at_disc_time = dvd_title(1, Some("03"));
        assert!(same_scan_title(&at_disc_time, &dvd_title(0, Some("3"))));
        assert!(!same_scan_title(&at_disc_time, &dvd_title(1, Some("04"))));
        let unnumbered = dvd_title(1, None);
        assert_eq!(unnumbered.source(), TitleSource::Unknown);
        assert!(!same_scan_title(&unnumbered, &unnumbered));
    }

    /// A synthetic bdmt file in the recorded form (a byte-order mark, CRLF line ends, the disclib wrapper, the name in
    /// `<di:title><di:name>`), with a made-up disc name and made-up thumbnail names (ruling RP21).
    const SYNTHETIC_BDMT: &str = concat!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n",
        "<disclib xmlns=\"urn:BDA:bdmv;disclib\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
        "xsi:schemaLocation=\"urn:BDA:bdmv;disclib disclib.xsd\">\r\n",
        "  <di:discinfo xmlns:di=\"urn:BDA:bdmv;discinfo\">\r\n",
        "    <di:title>\r\n",
        "      <di:name>Example Film</di:name>\r\n",
        "    </di:title>\r\n",
        "    <di:description>\r\n",
        "      <di:thumbnail href=\"./EXAMPLE_FILM_SMALL.jpg\" size=\"320x180\" />\r\n",
        "      <di:thumbnail href=\"./EXAMPLE_FILM_LARGE.jpg\" size=\"640x360\" />\r\n",
        "    </di:description>\r\n",
        "  </di:discinfo>\r\n",
        "</disclib>",
    );

    #[test]
    fn bdmt_name_reads_the_disc_name_of_a_bdmt_file_with_a_byte_order_mark_and_crlf_line_ends() {
        assert!(SYNTHETIC_BDMT.starts_with('\u{feff}') && SYNTHETIC_BDMT.contains("\r\n"));
        assert_eq!(bdmt_name(SYNTHETIC_BDMT).as_deref(), Some("Example Film"));
        assert_eq!(bdmt_name(SYNTHETIC_BDMT.trim_start_matches('\u{feff}')).as_deref(), Some("Example Film"));
    }

    #[test]
    fn bdmt_name_decodes_the_five_entities_and_numeric_character_references_once_and_trims() {
        assert_eq!(
            bdmt_name("<di:name>  Tom &amp; Jerry &lt;3&gt; &quot;Live&quot; &apos;84 </di:name>").as_deref(),
            Some("Tom & Jerry <3> \"Live\" '84")
        );
        assert_eq!(bdmt_name("<di:name>A &amp;lt; B</di:name>").as_deref(), Some("A &lt; B"));
        assert_eq!(bdmt_name("<di:name>Caf&#233; &#xE9; &copy; &</di:name>").as_deref(), Some("Café é &copy; &"));
    }

    #[test]
    fn bdmt_name_is_none_without_a_name_element_or_with_an_empty_one() {
        for xml in [
            "",
            "<di:title></di:title>",
            "<di:name></di:name>",
            "<di:name/>",
            "<di:name />",
            "<di:name>   </di:name>",
            "<di:name>never closed",
            "<di:name",
        ] {
            assert_eq!(bdmt_name(xml), None, "{xml:?}");
        }
    }

    #[test]
    fn bdmt_name_takes_the_first_name_element_with_or_without_attributes() {
        assert_eq!(
            bdmt_name("<di:names>Wrong</di:names><di:name xml:lang=\"en\">Right</di:name>").as_deref(),
            Some("Right")
        );
        assert_eq!(bdmt_name("<di:name>First</di:name><di:name>Second</di:name>").as_deref(), Some("First"));
    }

    #[test]
    fn bdmt_candidates_put_bdmt_eng_first_then_the_others_in_name_order() {
        let names: Vec<String> = [
            "bdmt_fra.xml",
            "EXAMPLE_FILM_SMALL.jpg",
            "bdmt_eng.xml",
            "bdmt_deu.xml",
            "BDMT_JPN.XML",
            "bdmt_eng.xml.bak",
            "bdmt_deu.xml",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect();
        assert_eq!(bdmt_candidates(&names), ["bdmt_eng.xml", "BDMT_JPN.XML", "bdmt_deu.xml", "bdmt_fra.xml"]);
        let upper = vec!["bdmt_fra.xml".to_string(), "BDMT_ENG.XML".to_string()];
        assert_eq!(bdmt_candidates(&upper), ["BDMT_ENG.XML", "bdmt_fra.xml"]);
        let english_only = vec!["bdmt_eng.xml".to_string()];
        assert_eq!(bdmt_candidates(&english_only), ["bdmt_eng.xml"]);
    }

    #[test]
    fn bdmt_candidates_are_empty_when_the_nav_tree_has_no_bdmt_file() {
        assert!(bdmt_candidates(&[]).is_empty());
        let dvd = vec!["VIDEO_TS.IFO".to_string(), "VTS_01_0.IFO".to_string()];
        assert!(bdmt_candidates(&dvd).is_empty());
    }
}
