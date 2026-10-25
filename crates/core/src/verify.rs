//! Verification of a ripped title against the scan and of a remuxed final file against the decision's layout (design
//! spec 6.2 step 7, 6.4 step 14, 9.4 messages 2003 and 4047; RP11). `verify_rip` checks MakeMKV's output: the duration
//! within 2 s (on a DVD from 2 s shorter to 1% longer), the chapter count, and the tracks as an ordered match on type,
//! output codec (attributes 41 and 42), language and channels, with a text-converted caption track expected right
//! after the audio and the tracks MakeMKV removed (`MSG:4047`, counted over the selection) not expected; a codec the
//! table does not know is not compared, and the report says so. Read errors (`MSG:2003`) are carried with their count
//! and offsets, never the drive's name. `verify_final` checks the remuxed file against the layout of 8.10, each track's
//! codec and channels against the scan stream it comes from, and its chapters against the staged file's. A report's
//! pairs map file track ids back to scan streams, so the subtitle evidence
//! of 6.4 step 10 and the text rule's decisions are keyed by scan stream. Built on `reconcile`'s row builders
//! (`expected_rows`, `removed_streams`), which stay the phase 0 rule, untouched (RP6). Written from the spec (RP20); no
//! I/O (RP19): callers hand in parsed values, text or a `Tree`. Used by Tasks 13, 16 and 18.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::codec::{mkvmerge_names, Family};
use crate::disc::{Flags, Stream, StreamKind, Title, DURATION_MATCH_S};
use crate::lang::norm_lang;
use crate::reconcile::{expected_rows, removed_streams};
use crate::recording::files::{MkvTrack, MkvmergeIdent, Packet, Selection, SubsIndexEntry};
use crate::recording::{read_text, Encoding, Errors, PathKey, Tree};
use crate::review::{sorted_unique, ReviewReason};
use crate::robot::{parse, Scan};
use crate::subtitle::align::{events, is_text, load_packets};
use crate::tracks::layout::{Layout, OutputTrack};

/// The scan streams a rip with `selection` kept, in scan order: the `stream` of every row of
/// `reconcile::expected_rows(scan, title, selection)` (3D video, attribute 6 containing "MVC", left out; Eng's filter of
/// eng/und audio without flags 256 and 1|2|4 and eng subtitles). MakeMKV's `MSG:4047` numbers count these streams from
/// 0. Err "the scan lists no title <n>" when the scan has no TINFO line for `title`, and `expected_rows`' sentence when
/// a stream's attribute 22 is not a number. Spec 6.2 step 7, RP11 ("built on reconcile's row builders").
pub fn selected_streams(scan: &Scan, title: u32, selection: Selection) -> Result<Vec<u32>, String> {
    if !scan.titles.contains_key(&title) {
        return Err(format!("the scan lists no title {title}"));
    }
    Ok(expected_rows(scan, title, selection)?.into_iter().map(|row| row.stream).collect())
}

/// `title` as the rip's file holds it: a copy keeping only the streams in `selection` (`selected_streams`' result for the
/// rip's selection), in scan order; the title's other facts are unchanged. The one copy of the narrowing that
/// `lazymkv-dev decide`'s decision section and `retune --subtitles`' 8.7 column both apply before 8.6 and 8.7 decide
/// (6.4 step 11), so the two judge a rip over the same streams, and `tracks::subtitle::step10_runs` counts only the
/// streams the rip kept. A stream MakeMKV removed (`MSG:4047`) stays: the caller passes the removals as facts. TheDiscDB's
/// track descriptions are aligned against the scan's streams as a whole (8.6 step 1), so a title narrowed by an eng
/// selection aligns no description (a deferred finding of the final review, E5); the product rips every stream but 3D
/// video (D6, 9.3), so phase 2's file phase decides over the whole title. Spec 6.2 step 7, 6.4 steps 10 and 11.
pub fn rip_title(title: &Title, selection: &[u32]) -> Title {
    let mut narrowed = title.clone();
    narrowed.streams.retain(|stream| selection.contains(&stream.number));
    narrowed
}

/// Attribute 41's value on a stream MakeMKV converts to text (a DVD's closed captions). Spec 6.2 step 7.
const TEXT_OUTPUT: &str = "Text";

/// A stream the file should hold, in file order (6.2 step 7).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExpectedTrack {
    /// The scan's stream number.
    pub stream: u32,
    /// Attribute 1.
    pub kind: StreamKind,
    /// The names `mkvmerge -J` may give the track's codec (`expected_codecs`); any one of them matches. An attribute 41
    /// value the table does not know expects its own text. Empty for a scan codec the table does not know (Family
    /// Unknown, without attribute 41): the codec is then not compared, and the report's `unchecked` says so.
    pub codecs: Vec<String>,
    /// Attribute 3 as `disc::Stream` holds it (lower-cased); None on video, whose language is never compared.
    pub language: Option<String>,
    /// Attribute 14 on an audio stream; None on video and subtitles.
    pub channels: Option<u32>,
    /// Attribute 22, carried onto the output track (9.5's MakeMKV-only flags).
    pub flags: Flags,
    /// Attribute 41 is "Text": the stream arrives as a text subtitle track, right after the audio.
    pub text_converted: bool,
}

/// The title's streams in `selection` (selected_streams' result) without those `removed` (MSG:4047 indices counted over
/// the selection from 0), with a text-converted subtitle stream (attribute 41 "Text") placed right after the audio
/// streams (`caption_first`). Codecs: `expected_codecs`, the names of `Stream.file_codec` (attribute 41, MakeMKV's
/// output codec short name, through `codec::output_codec` when present: "Text" gives ["SubRip/SRT"]); attribute 42
/// alone changes nothing. Language attribute 3 (None on video); flags carried (9.5's MakeMKV-only flags onto the output
/// track). A selected stream the title does not hold as a typed stream (no attribute 1) is not expected but keeps its
/// place in the count. Spec 6.2 step 7.
pub fn expected_tracks(title: &Title, selection: &[u32], removed: &BTreeSet<i64>) -> Vec<ExpectedTrack> {
    let mut tracks = Vec::new();
    for (index, number) in selection.iter().enumerate() {
        let position = i64::try_from(index).unwrap_or(i64::MAX);
        if removed.contains(&position) {
            continue;
        }
        if let Some(stream) = title.streams.iter().find(|stream| stream.number == *number) {
            tracks.push(expected_track(stream));
        }
    }
    caption_first(tracks)
}

/// The names `mkvmerge -J` may give the file's track of `stream`: `codec::mkvmerge_names` of `Stream.file_codec`
/// (attribute 41 through `codec::output_codec` when present, else attributes 5 to 7). A file codec the table does not
/// know expects attribute 41's own text when the stream has one, and nothing otherwise (the codec is then not
/// compared). Spec 6.2 step 7 "Output codec", 6.4 step 14.
pub fn expected_codecs(stream: &Stream) -> Vec<String> {
    if stream.file_codec.family == Family::Unknown {
        return stream.output_codec.iter().cloned().collect();
    }
    mkvmerge_names(&stream.file_codec).iter().map(|name| name.to_string()).collect()
}

/// One stream as the file should hold it. Spec 6.2 step 7 "Output codec".
fn expected_track(stream: &Stream) -> ExpectedTrack {
    ExpectedTrack {
        stream: stream.number,
        kind: stream.kind,
        codecs: expected_codecs(stream),
        language: if stream.kind == StreamKind::Video { None } else { stream.language.clone() },
        channels: if stream.kind == StreamKind::Audio { stream.channels } else { None },
        flags: stream.flags,
        text_converted: stream.output_codec.as_deref() == Some(TEXT_OUTPUT),
    }
}

/// Moves every text_converted track to right after the last audio track (6.2 step 7 "Order"); without audio, right
/// after the last video track; without either, first. The other tracks keep their order, and so do the text tracks.
pub fn caption_first(tracks: Vec<ExpectedTrack>) -> Vec<ExpectedTrack> {
    let (text, mut others): (Vec<ExpectedTrack>, Vec<ExpectedTrack>) =
        tracks.into_iter().partition(|track| track.text_converted);
    let after = others
        .iter()
        .rposition(|track| track.kind == StreamKind::Audio)
        .or_else(|| others.iter().rposition(|track| track.kind == StreamKind::Video))
        .map_or(0, |position| position + 1);
    for (offset, track) in text.into_iter().enumerate() {
        others.insert(after + offset, track);
    }
    others
}

/// How much longer than the scan's duration a DVD file may run: 1% (spec 6.2 step 7, provisional, set from the two
/// DVDs of phase 0; a fixed value of spec 14).
pub const DVD_LONGER_MAX: f64 = 0.01;

/// |file - expected| <= disc::DURATION_MATCH_S, or on a DVD expected - DURATION_MATCH_S <= file <= expected * 1.01 (6.2
/// step 7). The DVD band widens the 2 s band and never narrows it: a DVD title shorter than 200 s still gets 2 s longer.
pub fn duration_ok(expected_seconds: f64, file_seconds: f64, dvd: bool) -> bool {
    let band = f64::from(DURATION_MATCH_S);
    if (file_seconds - expected_seconds).abs() <= band {
        return true;
    }
    dvd && expected_seconds - band <= file_seconds && file_seconds <= expected_seconds * (1.0 + DVD_LONGER_MAX)
}

/// MSG:2003, a read error (spec 9.4).
const READ_ERROR: u32 = 2003;

/// One read-error offset of a rip log (6.2 step 7 "with the error count and offsets"). Not `recording::ReadError` (a
/// failed file read, the error of `Tree::read`); import it as `verify::ReadError`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReadError {
    /// The line's third parameter (`%3`, "1048576"); "" when the line has none.
    pub offset: String,
    /// The MSG:2003 lines at this offset.
    pub count: u32,
    /// The first line's first parameter at this offset (the `%1` error text); "" when the line has none.
    pub message: String,
}

/// Every MSG:2003 line's offset with the count of lines at that offset: `offset` is params[2]; `message` is params[0]
/// alone (the `%1` error text, "Scsi error - ILLEGAL REQUEST:READ OF SCRAMBLED SECTOR WITHOUT AUTHENTICATION"), the
/// first per offset; Message.text and params[1] (the drive's name and serial) are never kept (robot.rs: drive fields
/// never reach a golden). Offsets in order of first appearance; a line without parameters counts under "". Spec 6.2
/// step 7, 9.4.
pub fn read_errors(rip_log: &str) -> Vec<ReadError> {
    let mut errors: Vec<ReadError> = Vec::new();
    for line in parse(rip_log).messages.iter().filter(|message| message.code == READ_ERROR) {
        let offset = line.params.get(2).cloned().unwrap_or_default();
        match errors.iter_mut().find(|error| error.offset == offset) {
            Some(error) => error.count = error.count.saturating_add(1),
            None => errors.push(ReadError {
                offset,
                count: 1,
                message: line.params.first().cloned().unwrap_or_default(),
            }),
        }
    }
    errors
}

/// The sentence for a title without attribute 9. Spec 6.2 step 7.
const NO_DURATION: &str = "duration: the scan gives no duration (attribute 9)";

/// The sentence for a title the scan gives no stream for: nothing passes on empty input. Spec 6.2 step 7.
const NOTHING_EXPECTED: &str = "tracks: the scan predicts no track for this title";

/// What verification found. Spec 6.2 step 7 (`verify_rip`) and 6.4 step 14 (`verify_final`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VerifyReport {
    /// The file's duration against `expected_seconds` (`duration_ok` of 6.2 step 7 for a rip; the 2 s band for a remux).
    pub duration_ok: bool,
    /// The scan's attribute 9 (verify_rip; 0 when the scan gives none) or the staged file's duration (verify_final),
    /// seconds.
    pub expected_seconds: f64,
    /// `ident.container.properties.duration` in nanoseconds / 1e9.
    pub file_seconds: f64,
    /// The file's first chapter edition against the scan's attribute 8 (verify_rip), or against the staged file's count
    /// when the layout keeps the chapters and 0 when it does not (verify_final, 8.10 "Chapters are kept").
    pub chapters_ok: bool,
    /// The scan's attribute 8 (verify_rip); the staged file's count, or 0 when the layout keeps no chapters
    /// (verify_final).
    pub scan_chapters: u32,
    /// The file's first chapter edition's `num_entries` (0 without an edition).
    pub file_chapters: u32,
    /// The ordered match found no difference.
    pub tracks_ok: bool,
    /// (expected stream, file track id) in order; None where the file lacks it.
    pub pairs: Vec<(u32, Option<i64>)>,
    /// The file's tracks past the last expected one.
    pub extra_tracks: Vec<i64>,
    /// MSG:4047 indices at or beyond the selection's length, sorted (they fail nothing, as in `reconcile`).
    pub unknown_removed: Vec<i64>,
    /// One sentence per difference, in track order ("track 3: forced expected true, file false", "track 2: codec expected
    /// DTS-HD Master Audio, file AC-3"), then the duration ("duration: expected 5400 s, file 5320.0 s"), then the chapters.
    pub mismatches: Vec<String>,
    /// One sentence per comparison not made, in track order: the codec of a track whose scan codec the codec table does
    /// not know ("track 1: codec not compared, the codec table does not know the scan's codec (file Opus)"). They fail
    /// nothing, so a correct rip of such a stream is neither ripped again nor held (6.2 step 7).
    pub unchecked: Vec<String>,
    /// `read_errors` of the rip log (verify_rip); empty for a remux.
    pub read_errors: Vec<ReadError>,
    /// duration_ok, chapters_ok and tracks_ok; read errors never fail it (6.2 step 7: the title is kept).
    pub ok: bool,
}

/// What the ordered match of the expected tracks against the file's tracks found.
struct Matched {
    pairs: Vec<(u32, Option<i64>)>,
    extra_tracks: Vec<i64>,
    mismatches: Vec<String>,
    unchecked: Vec<String>,
}

/// What comparing one file track with the track expected at its position found: the differences, and the comparisons
/// that could not be made.
#[derive(Default)]
struct Compared {
    mismatches: Vec<String>,
    unchecked: Vec<String>,
}

/// The ordered match of 6.2 step 7 and 6.4 step 14, position by position: the expected track at position i against the
/// file's track at position i. A wrong type is one sentence and the track's other fields are not compared; otherwise
/// `differences` names each field that differs. An expected track past the file's last is paired with None; a file
/// track past the last expected one is extra. An empty expected list is the sentence `nothing` of its own (the caller's:
/// `NOTHING_EXPECTED` for a rip, whose tracks come from the scan; `NOTHING_KEPT` for a remux, whose come from the layout).
fn match_tracks<T>(
    expected: &[T],
    file: &[MkvTrack],
    nothing: &str,
    key: impl Fn(&T) -> (u32, StreamKind),
    differences: impl Fn(&T, &MkvTrack) -> Compared,
) -> Matched {
    let mut matched = Matched { pairs: Vec::new(), extra_tracks: Vec::new(), mismatches: Vec::new(), unchecked: Vec::new() };
    for (position, want) in expected.iter().enumerate() {
        let (stream, kind) = key(want);
        match file.get(position) {
            Some(track) => {
                matched.pairs.push((stream, Some(track.id)));
                if track.r#type != kind.as_str() {
                    matched
                        .mismatches
                        .push(format!("track {}: type expected {}, file {}", track.id, kind.as_str(), track.r#type));
                } else {
                    let compared = differences(want, track);
                    matched.mismatches.extend(compared.mismatches);
                    matched.unchecked.extend(compared.unchecked);
                }
            }
            None => {
                matched.pairs.push((stream, None));
                matched.mismatches.push(format!("stream {stream} ({}): missing from the file", kind.as_str()));
            }
        }
    }
    for track in file.iter().skip(expected.len()) {
        matched.extra_tracks.push(track.id);
        matched.mismatches.push(format!("track {}: not expected ({} {})", track.id, track.r#type, track.codec));
    }
    if expected.is_empty() {
        matched.mismatches.push(nothing.to_string());
    }
    matched
}

/// A file track's codec against `codecs`: any one of them matches; with `codecs` empty (a scan codec the table does not
/// know) nothing is compared and an `unchecked` sentence says so. Spec 6.2 step 7, 6.4 step 14.
fn codec_difference(codecs: &[String], track: &MkvTrack, found: &mut Compared) {
    if codecs.is_empty() {
        found.unchecked.push(format!(
            "track {}: codec not compared, the codec table does not know the scan's codec (file {})",
            track.id, track.codec
        ));
    } else if !codecs.iter().any(|name| *name == track.codec) {
        found.mismatches.push(format!("track {}: codec expected {}, file {}", track.id, codecs.join(" or "), track.codec));
    }
}

/// A file track's property `audio_channels` against an audio stream's attribute 14 (nothing compared when `channels`
/// is None: video, subtitles). Spec 6.2 step 7, 6.4 step 14.
fn channel_difference(channels: Option<u32>, track: &MkvTrack, found: &mut Compared) {
    let Some(channels) = channels else {
        return;
    };
    let got = track.properties.get("audio_channels").and_then(Value::as_i64);
    if got != Some(i64::from(channels)) {
        let shown = got.map_or_else(|| "none".to_string(), |count| count.to_string());
        found.mismatches.push(format!("track {}: channels expected {channels}, file {shown}", track.id));
    }
}

/// A file track's language property against `language` through `norm_lang` on both sides (never on video). Spec 6.2
/// step 7, 6.4 step 14.
fn language_difference(kind: StreamKind, language: Option<&str>, track: &MkvTrack, found: &mut Compared) {
    if kind == StreamKind::Video {
        return;
    }
    let (expected, got) = (norm_lang(language), file_language(track));
    if expected != got {
        found.mismatches.push(format!("track {}: language expected {expected}, file {got}", track.id));
    }
}

/// A rip's track against the scan's stream: codec (any of `codecs`; not compared for a codec the table does not know),
/// language (`norm_lang` on both sides; never on video) and channels (property `audio_channels`, on audio). Spec 6.2
/// step 7.
fn rip_differences(want: &ExpectedTrack, track: &MkvTrack) -> Compared {
    let mut found = Compared::default();
    codec_difference(&want.codecs, track, &mut found);
    language_difference(want.kind, want.language.as_deref(), track, &mut found);
    channel_difference(want.channels, track, &mut found);
    found
}

/// The track's `language` property through `norm_lang` ("und" when absent or not text).
fn file_language(track: &MkvTrack) -> String {
    norm_lang(track.properties.get("language").and_then(Value::as_str))
}

/// `ident.container.properties.duration` (nanoseconds) in seconds.
fn file_duration(ident: &MkvmergeIdent) -> f64 {
    ident.container.properties.duration as f64 / 1e9
}

/// The file's chapter count: the first edition's `num_entries`, 0 without an edition.
fn chapter_entries(ident: &MkvmergeIdent) -> i64 {
    ident.chapters.first().map_or(0, |edition| edition.num_entries)
}

/// "duration: expected <expected> s, file <file, one decimal> s".
fn duration_sentence(expected_seconds: f64, file_seconds: f64) -> String {
    format!("duration: expected {expected_seconds} s, file {file_seconds:.1} s")
}

/// 6.2 step 7: duration_ok against the title's seconds; chapters `ident.chapters.first().map_or(0, |c| c.num_entries)`
/// compared with `i64::from(title.chapters)`; the ordered match of expected_tracks (removed from
/// `reconcile::removed_streams` of `rip_log`) against ident.tracks on type, codec (any of `codecs`; an `unchecked`
/// sentence, not a difference, for a codec the table does not know), language
/// (`norm_lang` of property `language` on both sides; not on video) and channels (property `audio_channels`);
/// read_errors of `rip_log`. `selection` is selected_streams' result for the rip's profile; `dvd` is the disc's
/// `Disc::is_dvd`. Without a rip log nothing is removed and no read error is known.
pub fn verify_rip(title: &Title, ident: &MkvmergeIdent, selection: &[u32], rip_log: Option<&str>, dvd: bool) -> VerifyReport {
    let removed = rip_log.map(removed_streams).unwrap_or_default();
    let selected = i64::try_from(selection.len()).unwrap_or(i64::MAX);
    let unknown_removed: Vec<i64> = removed.iter().copied().filter(|&index| index >= selected).collect();
    let expected = expected_tracks(title, selection, &removed);
    let matched = match_tracks(
        &expected,
        &ident.tracks,
        NOTHING_EXPECTED,
        |track: &ExpectedTrack| (track.stream, track.kind),
        rip_differences,
    );
    let tracks_ok = matched.mismatches.is_empty();
    let mut mismatches = matched.mismatches;
    let expected_seconds = title.seconds.map_or(0.0, f64::from);
    let file_seconds = file_duration(ident);
    let duration_passes = title.seconds.is_some() && duration_ok(expected_seconds, file_seconds, dvd);
    if title.seconds.is_none() {
        mismatches.push(NO_DURATION.to_string());
    } else if !duration_passes {
        mismatches.push(duration_sentence(expected_seconds, file_seconds));
    }
    let entries = chapter_entries(ident);
    let chapters_ok = entries == i64::from(title.chapters);
    if !chapters_ok {
        mismatches.push(format!("chapters: expected {}, file {entries}", title.chapters));
    }
    VerifyReport {
        duration_ok: duration_passes,
        expected_seconds,
        file_seconds,
        chapters_ok,
        scan_chapters: title.chapters,
        file_chapters: u32::try_from(entries).unwrap_or(0),
        tracks_ok,
        pairs: matched.pairs,
        extra_tracks: matched.extra_tracks,
        unknown_removed,
        mismatches,
        unchecked: matched.unchecked,
        read_errors: rip_log.map(read_errors).unwrap_or_default(),
        ok: duration_passes && chapters_ok && tracks_ok,
    }
}

/// verify_failed when `!report.ok && second_failure`; read_errors when `report.read_errors` is not empty; sorted and
/// unique (`review::sorted_unique`). Verification lowers no confidence: these reasons hold the job (open point 30).
/// Spec 6.2 step 7, 8.8.
pub fn failure_reviews(report: &VerifyReport, second_failure: bool) -> Vec<ReviewReason> {
    let mut reasons = Vec::new();
    if !report.ok && second_failure {
        reasons.push(ReviewReason::VerifyFailed);
    }
    if !report.read_errors.is_empty() {
        reasons.push(ReviewReason::ReadErrors);
    }
    sorted_unique(reasons)
}

/// The sentence for an empty layout: nothing passes on empty input, and 6.4 step 14's expected tracks are the decision's
/// layout, not the scan's. Spec 6.4 step 14.
const NOTHING_KEPT: &str = "tracks: the layout keeps no track";

/// A flag property of the track (`default_track`, `forced_track`, `flag_hearing_impaired`, `flag_original`); absent or
/// not a bool is false.
fn file_flag(track: &MkvTrack, key: &str) -> bool {
    track.properties.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// A remuxed track against the layout's: codec and channels against the scan stream the layout names (`OutputTrack.stream`
/// in `title`: the names of `expected_codecs`, not compared for a codec the table does not know; property
/// `audio_channels` against attribute 14 on audio; a stream the title lacks is a difference), language (`norm_lang` on
/// both sides; never on video, whose language 8.10 does not set), name (`track_name`; absent is none), then the four
/// flags. Spec 6.4 step 14, 8.10.
fn final_differences(title: &Title, want: &OutputTrack, track: &MkvTrack) -> Compared {
    let mut found = Compared::default();
    match title.streams.iter().find(|stream| stream.number == want.stream) {
        Some(stream) => {
            codec_difference(&expected_codecs(stream), track, &mut found);
            let channels = if stream.kind == StreamKind::Audio { stream.channels } else { None };
            channel_difference(channels, track, &mut found);
        }
        None => found.mismatches.push(format!("track {}: stream {} is not a stream of the title", track.id, want.stream)),
    }
    language_difference(want.kind, want.language.as_deref(), track, &mut found);
    let name = track.properties.get("track_name").and_then(Value::as_str);
    if want.name.as_deref() != name {
        found.mismatches.push(format!(
            "track {}: name expected {}, file {}",
            track.id,
            want.name.as_deref().unwrap_or("none"),
            name.unwrap_or("none")
        ));
    }
    let flags = [
        ("default", want.default, "default_track"),
        ("forced", want.forced, "forced_track"),
        ("hearing_impaired", want.hearing_impaired, "flag_hearing_impaired"),
        ("original", want.original, "flag_original"),
    ];
    for (flag, expected, key) in flags {
        let got = file_flag(track, key);
        if got != expected {
            found.mismatches.push(format!("track {}: {flag} expected {expected}, file {got}", track.id));
        }
    }
    found
}

/// 6.4 step 14: the partial file's tracks must equal the layout exactly (order, type, language through norm_lang of
/// property `language`, name from `track_name`, flags from the properties `default_track`, `forced_track`,
/// `flag_hearing_impaired`, `flag_original`, an absent key being false), each track's codec and channels must be those
/// of the scan stream of `title` it comes from (`final_differences`), its duration be within disc::DURATION_MATCH_S of
/// the staged file's (`staged`, the rip's `verify_rip` report: the remux reads that file, so the DVD band does not
/// apply), and its chapter count equal the staged file's when `layout.chapters_kept` (8.10 "Chapters are kept"), or 0
/// when not; each difference is a `mismatches` sentence and each comparison not made an `unchecked` one. The video
/// track's language is not compared; `unknown_removed` and `read_errors` are empty. An empty layout never verifies
/// ("tracks: the layout keeps no track").
pub fn verify_final(layout: &Layout, title: &Title, staged: &VerifyReport, ident: &MkvmergeIdent) -> VerifyReport {
    let matched = match_tracks(
        &layout.tracks,
        &ident.tracks,
        NOTHING_KEPT,
        |track: &OutputTrack| (track.stream, track.kind),
        |want: &OutputTrack, track: &MkvTrack| final_differences(title, want, track),
    );
    let tracks_ok = matched.mismatches.is_empty();
    let mut mismatches = matched.mismatches;
    let source_seconds = staged.file_seconds;
    let file_seconds = file_duration(ident);
    let duration_passes = (file_seconds - source_seconds).abs() <= f64::from(DURATION_MATCH_S);
    if !duration_passes {
        mismatches.push(duration_sentence(source_seconds, file_seconds));
    }
    let expected_chapters = if layout.chapters_kept { staged.file_chapters } else { 0 };
    let entries = chapter_entries(ident);
    let chapters_ok = entries == i64::from(expected_chapters);
    if !chapters_ok {
        mismatches.push(format!("chapters: expected {expected_chapters}, file {entries}"));
    }
    VerifyReport {
        duration_ok: duration_passes,
        expected_seconds: source_seconds,
        file_seconds,
        chapters_ok,
        scan_chapters: expected_chapters,
        file_chapters: u32::try_from(entries).unwrap_or(0),
        tracks_ok,
        pairs: matched.pairs,
        extra_tracks: matched.extra_tracks,
        unknown_removed: Vec::new(),
        mismatches,
        unchecked: matched.unchecked,
        read_errors: Vec::new(),
        ok: duration_passes && chapters_ok && tracks_ok,
    }
}

/// The scan stream a file track id stands for, through `pairs` (an ffprobe index of an MKV equals its mkvmerge track
/// id); None for a track no pair holds, and None for every track when the track match failed (`tracks_ok` false: its
/// pairs are positions, not matches). Spec 6.2 step 7, 6.4 step 10.
pub fn stream_of_track(report: &VerifyReport, track_id: i64) -> Option<u32> {
    if !report.tracks_ok {
        return None;
    }
    report.pairs.iter().find(|(_, track)| *track == Some(track_id)).map(|(stream, _)| *stream)
}

/// The scan streams MakeMKV removed (6.2 step 7): each MSG:4047 index of reconcile::removed_streams(rip_log) counts the
/// selection in scan order; an index beyond it names no stream. Spec 6.2 step 7 "Removed streams", 9.4 message 4047.
pub fn removed_scan_streams(rip_log: Option<&str>, selection: &[u32]) -> BTreeSet<u32> {
    rip_log
        .map(removed_streams)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|index| usize::try_from(index).ok().and_then(|i| selection.get(i).copied()))
        .collect()
}

/// subs/index.json's `n` to the scan stream, through each entry's `track_id` and `stream_of_track`; entries without a
/// track_id or a pair are left out, so a report whose track match failed maps none. Spec 6.4 step 10, 8.7.
pub fn subs_streams(index: &[SubsIndexEntry], report: &VerifyReport) -> BTreeMap<i64, u32> {
    index
        .iter()
        .filter_map(|entry| {
            let stream = stream_of_track(report, entry.track_id?)?;
            Some((entry.n, stream))
        })
        .collect()
}

/// subs_dir/index.json as `align::index_streams` reads it: UTF-8 with a byte-order mark allowed, invalid bytes
/// replaced. Err is the read error's text (it names the path) or "<path>: <serde error>". The one reading
/// `subtitle_events` and decide's text decisions share. Spec 6.4 step 10, 8.7.
pub fn subs_index(tree: &dyn Tree, subs_dir: &PathKey) -> Result<Vec<SubsIndexEntry>, String> {
    let path = subs_dir.join("index.json");
    let text = read_text(tree, &path, Encoding::Utf8Sig, Errors::Replace)?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", tree.display(&path)))
}

/// One sub-<n>.packets.json as read for the subtitle-event rule: `shown` is the tree's display of its path, and
/// `packets` the packets or the error text `subtitle_events` gives for it (a missing file: the read error, which names
/// the path; any other read, decode, syntax or shape error: prefixed with `shown` exactly once). Not
/// `recording::files::PacketsFile` (files.rs:175), the file's serde shape. Spec 6.4 step 10, 8.7, K5. Def: T18. Use: T19.
#[derive(Clone, Debug)]
pub struct PacketsFile {
    pub shown: String,
    pub packets: Result<Vec<Packet>, String>,
}

/// `subs_dir/sub-<n>.packets.json` read strictly (UTF-8, a byte-order mark allowed) and parsed by `align::load_packets`,
/// its error held in `packets` as `PacketsFile` says. Spec 6.4 step 10, K5. Def: T18. Use: T18, T19.
pub fn read_packets_file(tree: &dyn Tree, subs_dir: &PathKey, n: i64) -> PacketsFile {
    let path = subs_dir.join(&format!("sub-{n}.packets.json"));
    let shown = tree.display(&path);
    let packets = read_text(tree, &path, Encoding::Utf8Sig, Errors::Strict)
        .map_err(|error| if tree.is_file(&path) { format!("{shown}: {error}") } else { error })
        .and_then(|text| {
            // load_packets prefixes its syntax and serde errors with `shown` but returns the AttributeError text of a
            // top level that is not an object bare, so that one gets the path here.
            load_packets(&text, &shown)
                .map_err(|error| if error.starts_with(&shown) { error } else { format!("{shown}: {error}") })
        });
    PacketsFile { shown, packets }
}

/// The one subtitle-event rule over read inputs: for each `index` entry in order that `subs_streams` maps, the
/// `align::events` of `packets[n]` at `clear_max_bytes` (0 where `align::is_text`), keyed by scan stream. Unmapped
/// entries are skipped, so their files may be broken or absent. A mapped entry's held error is returned as is, its
/// events error as "<shown>: <error>", and a mapped entry with no `packets` entry as "sub-<n>.packets.json: not read".
/// Spec 6.4 step 10, 8.7, K5, S25. Def: T18. Use: T18, T19.
pub fn subtitle_events_of(
    index: &[SubsIndexEntry],
    packets: &BTreeMap<i64, PacketsFile>,
    report: &VerifyReport,
    clear_max_bytes: i64,
) -> Result<BTreeMap<u32, Vec<f64>>, String> {
    let streams = subs_streams(index, report);
    let mut by_stream = BTreeMap::new();
    for entry in index {
        let Some(&stream) = streams.get(&entry.n) else {
            continue;
        };
        let file = packets.get(&entry.n).ok_or_else(|| format!("sub-{}.packets.json: not read", entry.n))?;
        let read = file.packets.as_ref().map_err(Clone::clone)?;
        let limit = if is_text(entry.codec.as_deref()) { 0 } else { clear_max_bytes };
        let starts = events(read, limit).map_err(|error| format!("{}: {error}", file.shown))?;
        by_stream.insert(stream, starts);
    }
    Ok(by_stream)
}

/// `align::events` of every subs/index.json entry under `subs_dir` (packets through `align::load_packets`), keyed by
/// scan stream (`subs_streams`): `clear_max_bytes` (spec 14 `sub_clear_max_bytes`) for image codecs, 0 where
/// `align::is_text`. index.json is read as `align::index_streams` reads it (a byte-order mark allowed, invalid bytes
/// replaced), each packets file strictly (`read_packets_file`); only mapped entries are read. Err with the read error's
/// text for a missing file (it names the path), or with the file's path before a decode error's, serde's or
/// `align::events`' message, so a failure always names its file. The reading half of the rule `subtitle_events_of`
/// holds. Spec 6.4 step 10, 8.7, K5. Def: phase 1, rewritten over `subtitle_events_of` in T18. Use:
/// `subtitle::labels::row14` (retune --subtitles).
pub fn subtitle_events(
    tree: &dyn Tree,
    subs_dir: &PathKey,
    report: &VerifyReport,
    clear_max_bytes: i64,
) -> Result<BTreeMap<u32, Vec<f64>>, String> {
    let index = subs_index(tree, subs_dir)?;
    let streams = subs_streams(&index, report);
    let packets: BTreeMap<i64, PacketsFile> = index
        .iter()
        .filter(|entry| streams.contains_key(&entry.n))
        .map(|entry| (entry.n, read_packets_file(tree, subs_dir, entry.n)))
        .collect();
    subtitle_events_of(&index, &packets, report, clear_max_bytes)
}

/// A map keyed by subs/index.json's `n` (such as `text::decide`'s decisions) re-keyed by scan stream through `streams`
/// (subs_streams' result); keys without a stream are left out. Spec 8.7 "Text".
pub fn by_scan_stream<T: Clone>(by_n: &BTreeMap<i64, T>, streams: &BTreeMap<i64, u32>) -> BTreeMap<u32, T> {
    by_n.iter().filter_map(|(n, value)| streams.get(n).map(|stream| (*stream, value.clone()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::{json, Value};

    use crate::disc::{disc, Flags, StreamKind, Title};
    use crate::recording::files::{MkvmergeIdent, Selection, SubsIndexEntry};
    use crate::recording::{MemTree, PathKey};
    use crate::review::ReviewReason;
    use crate::robot::{parse, Scan};
    use crate::settings::Settings;
    use crate::testsupport::scan_text;
    use crate::tracks::layout::{Layout, OutputTrack};

    /// Attribute 42 of every stream MakeMKV does not convert, as the recordings write it.
    const LOSSLESS: &str = "( Lossless conversion )";
    /// Attribute 42 of the converted closed-caption stream (short-circuit-2-1988-dvd's stream 12).
    const LOSSY: &str = "( Lossy conversion )";

    /// A Blu-ray title 0 shaped like pans-labyrinth-2006-uhd's feature (its scan.txt, 2026-10-16): HEVC video; Spanish
    /// DTS-HD MA 5.1 (flags 1024) and its derived DTS core (2304); English Dolby Digital 2.0; English, English derived
    /// forced-only (6144), Spanish and Spanish derived forced-only PGS subtitles; 22 chapters; 1:59:10.
    fn bluray_scan() -> Scan {
        parse(&scan_text(
            &[(1, "Blu-ray disc")],
            &[(
                0,
                &[(8, "22"), (9, "1:59:10")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEGH/ISO/HEVC"), (6, "MpegH"), (7, "MpegH HEVC Main10@L5.1"), (22, "0"), (42, LOSSLESS)]),
                    (1, &[(1, "Audio"), (3, "spa"), (5, "A_DTS"), (6, "DTS-HD MA"), (7, "DTS-HD Master Audio"), (14, "6"), (22, "1024"), (42, LOSSLESS)]),
                    (2, &[(1, "Audio"), (3, "spa"), (5, "A_DTS"), (6, "DTS"), (7, "DTS"), (14, "6"), (22, "2304"), (42, LOSSLESS)]),
                    (3, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2"), (22, "0"), (42, LOSSLESS)]),
                    (4, &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles"), (22, "0"), (42, LOSSLESS)]),
                    (5, &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles"), (22, "6144"), (42, LOSSLESS)]),
                    (6, &[(1, "Subtitles"), (3, "spa"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles"), (22, "0"), (42, LOSSLESS)]),
                    (7, &[(1, "Subtitles"), (3, "spa"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles"), (22, "6144"), (42, LOSSLESS)]),
                ],
            )],
        ))
    }

    /// A DVD title 0 shaped like short-circuit-2-1988-dvd's feature (its scan.txt, 2026-10-16), with fewer languages:
    /// MPEG-2 video; English and French Dolby Digital 2.0; English and French VobSub subtitles; the English closed
    /// captions MakeMKV converts to text (attribute 41 "Text", 42 lossy), listed last; 28 chapters; 1:50:06.
    fn dvd_scan() -> Scan {
        parse(&scan_text(
            &[(1, "DVD disc")],
            &[(
                0,
                &[(8, "28"), (9, "1:50:06"), (24, "01")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG2"), (6, "Mpeg2"), (7, "Mpeg2"), (22, "0"), (42, LOSSLESS)]),
                    (1, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2"), (22, "0"), (42, LOSSLESS)]),
                    (2, &[(1, "Audio"), (3, "fre"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2"), (22, "0"), (42, LOSSLESS)]),
                    (3, &[(1, "Subtitles"), (3, "eng"), (5, "S_VOBSUB"), (6, ""), (7, "Dvd Subtitles"), (22, "0"), (42, LOSSLESS)]),
                    (4, &[(1, "Subtitles"), (3, "fre"), (5, "S_VOBSUB"), (6, ""), (7, "Dvd Subtitles"), (22, "0"), (42, LOSSLESS)]),
                    (5, &[(1, "Subtitles"), (3, "eng"), (5, "S_CC608/DVD"), (6, "CC"), (7, "Closed Captions"), (22, "0"), (41, "Text"), (42, LOSSY)]),
                ],
            )],
        ))
    }

    /// A Blu-ray title 0 with an Atmos and a plain TrueHD track: AVC video, English TrueHD Atmos 7.1, English TrueHD 5.1;
    /// 2 chapters; 0:10:00.
    fn truehd_scan() -> Scan {
        parse(&scan_text(
            &[(1, "Blu-ray disc")],
            &[(
                0,
                &[(8, "2"), (9, "0:10:00")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1"), (22, "0"), (42, LOSSLESS)]),
                    (1, &[(1, "Audio"), (3, "eng"), (5, "A_TRUEHD"), (6, "TrueHD"), (7, "TrueHD Atmos"), (14, "8"), (22, "0"), (42, LOSSLESS)]),
                    (2, &[(1, "Audio"), (3, "eng"), (5, "A_TRUEHD"), (6, "TrueHD"), (7, "TrueHD"), (14, "6"), (22, "0"), (42, LOSSLESS)]),
                ],
            )],
        ))
    }

    /// Title `number` of `scan`, typed.
    fn title_of(scan: &Scan, number: u32) -> Title {
        disc(scan).unwrap().title(number).unwrap().clone()
    }

    #[test]
    fn selected_streams_for_all_are_every_stream_of_the_title_in_scan_order() {
        assert_eq!(selected_streams(&bluray_scan(), 0, Selection::All), Ok(vec![0, 1, 2, 3, 4, 5, 6, 7]));
        assert_eq!(selected_streams(&dvd_scan(), 0, Selection::All), Ok(vec![0, 1, 2, 3, 4, 5]));
    }

    #[test]
    fn selected_streams_for_eng_keep_english_and_no_language_audio_without_core_or_commentary_flags() {
        // Title 2: video; English DTS-HD MA (1024, kept), its derived core (2304: flag 256, dropped), English Dolby
        // Digital flagged director's comments (1, dropped), audio without a language (und, kept), Spanish audio
        // (dropped); English (kept) and French (dropped) subtitles.
        let scan = parse(&scan_text(
            &[(1, "Blu-ray disc")],
            &[(
                2,
                &[(8, "1"), (9, "0:30:00")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")]),
                    (1, &[(1, "Audio"), (3, "eng"), (5, "A_DTS"), (6, "DTS-HD MA"), (7, "DTS-HD Master Audio"), (14, "6"), (22, "1024")]),
                    (2, &[(1, "Audio"), (3, "eng"), (5, "A_DTS"), (6, "DTS"), (7, "DTS"), (14, "6"), (22, "2304")]),
                    (3, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2"), (22, "1")]),
                    (4, &[(1, "Audio"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")]),
                    (5, &[(1, "Audio"), (3, "spa"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")]),
                    (6, &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")]),
                    (7, &[(1, "Subtitles"), (3, "fre"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")]),
                ],
            )],
        ));
        assert_eq!(selected_streams(&scan, 2, Selection::Eng), Ok(vec![0, 1, 4, 6]));
        assert_eq!(selected_streams(&scan, 2, Selection::All), Ok(vec![0, 1, 2, 3, 4, 5, 6, 7]));
        // pans-labyrinth-2006-uhd's eng-only rip kept the English audio and both English subtitle streams.
        assert_eq!(selected_streams(&bluray_scan(), 0, Selection::Eng), Ok(vec![0, 3, 4, 5]));
        // short-circuit-2-1988-dvd's eng-only rip kept the English VobSub and the English closed captions.
        assert_eq!(selected_streams(&dvd_scan(), 0, Selection::Eng), Ok(vec![0, 1, 3, 5]));
    }

    #[test]
    fn a_3d_video_stream_is_never_selected() {
        let scan = parse(&scan_text(
            &[],
            &[(
                0,
                &[(8, "1"), (9, "0:10:00")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")]),
                    (1, &[(1, "Video"), (5, "V_MPEG4/ISO/MVC"), (6, "Mpeg4 MVC"), (7, "Mpeg4 MVC High@L4.1")]),
                    (2, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")]),
                ],
            )],
        ));
        assert_eq!(selected_streams(&scan, 0, Selection::All), Ok(vec![0, 2]));
        assert_eq!(selected_streams(&scan, 0, Selection::Eng), Ok(vec![0, 2]));
    }

    #[test]
    fn rip_title_keeps_only_the_selected_streams_in_scan_order() {
        let title = title_of(&bluray_scan(), 0);
        let selection = selected_streams(&bluray_scan(), 0, Selection::Eng).unwrap();
        let narrowed = rip_title(&title, &selection);
        assert_eq!(narrowed.streams.iter().map(|stream| stream.number).collect::<Vec<u32>>(), vec![0, 3, 4, 5]);
        assert_eq!((narrowed.number, narrowed.seconds, narrowed.chapters), (title.number, title.seconds, title.chapters));
        assert_eq!(rip_title(&title, &[0, 1, 2, 3, 4, 5, 6, 7]), title);
    }

    #[test]
    fn selected_streams_refuse_a_title_the_scan_lacks() {
        assert_eq!(selected_streams(&bluray_scan(), 7, Selection::All), Err("the scan lists no title 7".to_string()));
    }

    #[test]
    fn a_flag_that_is_not_a_number_is_refused_with_reconciles_sentence() {
        let scan = parse(&scan_text(
            &[],
            &[(
                0,
                &[(8, "1"), (9, "0:10:00")],
                &[(0, &[(1, "Video"), (6, "Mpeg2")]), (1, &[(1, "Audio"), (3, "eng"), (6, "DD"), (22, "x")])],
            )],
        ));
        assert_eq!(
            selected_streams(&scan, 0, Selection::All),
            Err("scan stream 0,1: attribute 22: invalid literal for int() with base 10: 'x'".to_string())
        );
    }

    /// An expected track with one codec name and no text conversion.
    fn expected(stream: u32, kind: StreamKind, codec: &str, language: Option<&str>, channels: Option<u32>, flags: u32) -> ExpectedTrack {
        ExpectedTrack {
            stream,
            kind,
            codecs: vec![codec.to_string()],
            language: language.map(str::to_string),
            channels,
            flags: Flags(flags),
            text_converted: false,
        }
    }

    /// A track for `caption_first`: only its stream, kind and text conversion matter there.
    fn bare(stream: u32, kind: StreamKind, text_converted: bool) -> ExpectedTrack {
        ExpectedTrack { stream, kind, codecs: Vec::new(), language: None, channels: None, flags: Flags(0), text_converted }
    }

    /// The streams `expected_tracks` gives for `selection` with the MSG:4047 indices `removed`.
    fn expected_streams(title: &Title, selection: &[u32], removed: &[i64]) -> Vec<u32> {
        let removed: BTreeSet<i64> = removed.iter().copied().collect();
        expected_tracks(title, selection, &removed).iter().map(|track| track.stream).collect()
    }

    #[test]
    fn expected_tracks_carry_mkvmerges_codec_names_languages_channels_and_flags() {
        let title = title_of(&bluray_scan(), 0);
        assert_eq!(
            expected_tracks(&title, &[0, 1, 2, 3, 4, 5, 6, 7], &BTreeSet::new()),
            vec![
                expected(0, StreamKind::Video, "HEVC/H.265/MPEG-H", None, None, 0),
                expected(1, StreamKind::Audio, "DTS-HD Master Audio", Some("spa"), Some(6), 1024),
                ExpectedTrack {
                    codecs: vec!["DTS".to_string(), "DTS-ES".to_string(), "DTS 96/24".to_string()],
                    ..expected(2, StreamKind::Audio, "DTS", Some("spa"), Some(6), 2304)
                },
                expected(3, StreamKind::Audio, "AC-3", Some("eng"), Some(2), 0),
                expected(4, StreamKind::Subtitles, "HDMV PGS", Some("eng"), None, 0),
                expected(5, StreamKind::Subtitles, "HDMV PGS", Some("eng"), None, 6144),
                expected(6, StreamKind::Subtitles, "HDMV PGS", Some("spa"), None, 0),
                expected(7, StreamKind::Subtitles, "HDMV PGS", Some("spa"), None, 6144),
            ]
        );
    }

    #[test]
    fn a_removed_track_is_counted_over_the_selection_not_the_scan() {
        let title = title_of(&bluray_scan(), 0);
        // pans-labyrinth-2006-uhd's eng-only rip.txt: "#3" is the fourth selected stream, scan stream 5.
        assert_eq!(expected_streams(&title, &[0, 3, 4, 5], &[3]), [0, 3, 4]);
        // Counted over every stream, the same "#3" would remove scan stream 3, an audio stream.
        assert_eq!(expected_streams(&title, &[0, 1, 2, 3, 4, 5, 6, 7], &[3]), [0, 1, 2, 4, 5, 6, 7]);
        // Its all-tracks rip.txt: "#5" and "#7" are the two derived forced-only streams.
        assert_eq!(expected_streams(&title, &[0, 1, 2, 3, 4, 5, 6, 7], &[5, 7]), [0, 1, 2, 3, 4, 6]);
    }

    #[test]
    fn a_text_converted_caption_track_is_expected_after_the_audio_and_before_the_bitmap_subtitles() {
        let title = title_of(&dvd_scan(), 0);
        assert_eq!(expected_streams(&title, &[0, 1, 2, 3, 4, 5], &[]), [0, 1, 2, 5, 3, 4]);
        assert_eq!(expected_streams(&title, &[0, 1, 3, 5], &[]), [0, 1, 5, 3]);
    }

    #[test]
    fn a_caption_stream_converted_to_text_is_expected_as_subrip_srt() {
        let title = title_of(&dvd_scan(), 0);
        assert_eq!(
            expected_tracks(&title, &[5], &BTreeSet::new()),
            vec![ExpectedTrack {
                stream: 5,
                kind: StreamKind::Subtitles,
                codecs: vec!["SubRip/SRT".to_string()],
                language: Some("eng".to_string()),
                channels: None,
                flags: Flags(0),
                text_converted: true,
            }]
        );
        assert_eq!(
            expected_tracks(&title, &[3], &BTreeSet::new()),
            vec![expected(3, StreamKind::Subtitles, "VobSub", Some("eng"), None, 0)]
        );
    }

    #[test]
    fn an_lpcm_stream_converted_to_flac_expects_a_flac_track() {
        // Provisional, the spelling not yet seen in a recording: an LPCM stream MakeMKV's default profile writes as FLAC
        // (attribute 41, the output codec short name, "FLAC"), beside the same stream kept as LPCM, and a third whose
        // attribute 41 the table does not know (a made-up "Made Up"), which expects the attribute 41 text itself.
        let scan = parse(&scan_text(
            &[(1, "Blu-ray disc")],
            &[(
                0,
                &[(8, "1"), (9, "0:10:00")],
                &[
                    (0, &[(1, "Audio"), (3, "eng"), (5, "A_PCM/INT/LIT"), (6, "LPCM"), (7, "LPCM"), (14, "6"), (41, "FLAC"), (42, LOSSLESS)]),
                    (1, &[(1, "Audio"), (3, "eng"), (5, "A_PCM/INT/LIT"), (6, "LPCM"), (7, "LPCM"), (14, "6"), (42, LOSSLESS)]),
                    (2, &[(1, "Audio"), (3, "eng"), (5, "A_PCM/INT/LIT"), (6, "LPCM"), (7, "LPCM"), (14, "6"), (41, "Made Up"), (42, LOSSLESS)]),
                ],
            )],
        ));
        let title = title_of(&scan, 0);
        let expected_tracks = expected_tracks(&title, &[0, 1, 2], &BTreeSet::new());
        assert_eq!(
            expected_tracks,
            vec![
                expected(0, StreamKind::Audio, "FLAC", Some("eng"), Some(6), 0),
                expected(1, StreamKind::Audio, "PCM", Some("eng"), Some(6), 0),
                expected(2, StreamKind::Audio, "Made Up", Some("eng"), Some(6), 0),
            ]
        );
        assert_eq!(expected_tracks[0].codecs, ["FLAC"]);
        let tracks = vec![
            track(0, "audio", "FLAC", json!({"language": "eng", "audio_channels": 6})),
            track(1, "audio", "PCM", json!({"language": "eng", "audio_channels": 6})),
            track(2, "audio", "Made Up", json!({"language": "eng", "audio_channels": 6})),
        ];
        let report = verify_rip(&title, &file(tracks, &[1], 600_000_000_000), &[0, 1, 2], None, false);
        assert_eq!(report.mismatches, Vec::<String>::new());
        assert!(report.ok);
    }

    #[test]
    fn attribute_42_alone_never_changes_the_expected_codec() {
        let lossless = title_of(&bluray_scan(), 0);
        assert_eq!(expected_tracks(&lossless, &[1], &BTreeSet::new())[0].codecs, ["DTS-HD Master Audio"]);
        // Not seen in a recording: a lossy conversion named without attribute 41.
        let scan = parse(&scan_text(
            &[],
            &[(
                0,
                &[(8, "1"), (9, "0:10:00")],
                &[(0, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6"), (42, LOSSY)])],
            )],
        ));
        let lossy = expected_tracks(&title_of(&scan, 0), &[0], &BTreeSet::new());
        assert_eq!(lossy, vec![expected(0, StreamKind::Audio, "AC-3", Some("eng"), Some(6), 0)]);
    }

    #[test]
    fn a_truehd_stream_accepts_both_of_mkvmerges_truehd_names() {
        let title = title_of(&truehd_scan(), 0);
        let tracks = expected_tracks(&title, &[1, 2], &BTreeSet::new());
        assert_eq!(tracks[0].codecs, ["TrueHD", "TrueHD Atmos"]);
        assert_eq!(tracks[1].codecs, ["TrueHD", "TrueHD Atmos"]);
        assert_eq!((tracks[0].channels, tracks[1].channels), (Some(8), Some(6)));
    }

    #[test]
    fn a_selected_stream_without_a_kind_is_not_expected_but_keeps_its_number() {
        // Stream 1 has no attribute 1: expected_rows selects it, disc::Title leaves it out.
        let scan = parse(&scan_text(
            &[],
            &[(
                0,
                &[(8, "1"), (9, "0:10:00")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG2"), (6, "Mpeg2"), (7, "Mpeg2")]),
                    (1, &[(3, "eng"), (6, "DD")]),
                    (2, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")]),
                ],
            )],
        ));
        assert_eq!(selected_streams(&scan, 0, Selection::All), Ok(vec![0, 1, 2]));
        let title = title_of(&scan, 0);
        assert_eq!(title.streams.len(), 2);
        assert_eq!(expected_streams(&title, &[0, 1, 2], &[]), [0, 2]);
        assert_eq!(expected_streams(&title, &[0, 1, 2], &[1]), [0, 2]);
        assert_eq!(expected_streams(&title, &[0, 1, 2], &[2]), [0]);
    }

    #[test]
    fn caption_first_keeps_the_other_tracks_in_order_and_several_text_tracks_in_theirs() {
        let tracks = vec![
            bare(0, StreamKind::Video, false),
            bare(1, StreamKind::Audio, false),
            bare(9, StreamKind::Subtitles, true),
            bare(2, StreamKind::Audio, false),
            bare(3, StreamKind::Subtitles, false),
            bare(8, StreamKind::Subtitles, true),
        ];
        let order: Vec<u32> = caption_first(tracks).iter().map(|track| track.stream).collect();
        assert_eq!(order, [0, 1, 2, 9, 8, 3]);
    }

    #[test]
    fn caption_first_without_audio_puts_the_text_track_after_the_video() {
        let tracks = vec![bare(0, StreamKind::Video, false), bare(3, StreamKind::Subtitles, false), bare(5, StreamKind::Subtitles, true)];
        let order: Vec<u32> = caption_first(tracks).iter().map(|track| track.stream).collect();
        assert_eq!(order, [0, 5, 3]);
    }

    #[test]
    fn caption_first_without_audio_or_video_puts_the_text_track_first() {
        let tracks = vec![bare(3, StreamKind::Subtitles, false), bare(5, StreamKind::Subtitles, true), bare(4, StreamKind::Subtitles, false)];
        let order: Vec<u32> = caption_first(tracks).iter().map(|track| track.stream).collect();
        assert_eq!(order, [5, 3, 4]);
    }

    #[test]
    fn duration_ok_accepts_two_seconds_either_way_on_a_blu_ray() {
        assert!(duration_ok(7150.0, 7150.1, false));
        assert!(duration_ok(7150.0, 7152.0, false));
        assert!(duration_ok(7150.0, 7148.0, false));
        assert!(!duration_ok(7150.0, 7152.5, false));
        assert!(!duration_ok(7150.0, 7147.5, false));
    }

    #[test]
    fn duration_ok_on_a_dvd_accepts_two_seconds_shorter_to_one_percent_longer() {
        assert!(duration_ok(6606.0, 6604.0, true));
        assert!(!duration_ok(6606.0, 6603.5, true));
        assert!(duration_ok(6606.0, 6672.0, true));
        assert!(!duration_ok(6606.0, 6673.0, true));
    }

    #[test]
    fn the_dvd_band_keeps_the_two_second_band_for_a_short_title() {
        // 1% of 100 s is 1 s, less than the 2 s every format accepts.
        assert!(duration_ok(100.0, 101.9, true));
        assert!(!duration_ok(100.0, 102.5, true));
        assert!(duration_ok(100.0, 98.0, true));
        assert!(!duration_ok(100.0, 97.5, true));
    }

    #[test]
    fn the_recorded_dvd_durations_pass_only_the_dvd_band() {
        assert!(duration_ok(6606.0, 6623.6, true));
        assert!(!duration_ok(6606.0, 6623.6, false));
        assert!(duration_ok(5904.0, 5919.9, true));
        assert!(!duration_ok(5904.0, 5919.9, false));
    }

    #[test]
    fn the_duration_constants_are_the_specs() {
        assert_eq!(DURATION_MATCH_S, 2);
        assert_eq!(DVD_LONGER_MAX, 0.01);
    }

    /// A made-up drive name; a recorded one carries the drive's serial and is never written into a test.
    const DRIVE: &str = "BD-RE EXAMPLE DRIVE 1.00 SERIAL0001";
    /// The read error short-circuit-2-1988-dvd's rip logged (spec 6.2 step 7).
    const SCRAMBLED: &str = "Scsi error - ILLEGAL REQUEST:READ OF SCRAMBLED SECTOR WITHOUT AUTHENTICATION";

    /// A MSG:2003 line as MakeMKV writes it, with `error` read from DRIVE at `offset`.
    fn read_error_line(error: &str, offset: &str) -> String {
        format!(
            "MSG:2003,0,3,\"Error '{error}' occurred while reading '{DRIVE}' at offset '{offset}'\",\"Error '%1' occurred while reading '%2' at offset '%3'\",\"{error}\",\"{DRIVE}\",\"{offset}\"\n"
        )
    }

    /// MakeMKV's MSG:4047 line for output track `n`, removed as an empty forced-subtitle track, as the recorded rips
    /// write it.
    fn removal(n: u32) -> String {
        format!(
            "MSG:4047,16,3,\"AV sync issue in stream {n} at 0:00:00 : Forced subtitles track #{n} turned out to be empty and was removed from output file\",\"AV sync issue in stream %1 at %2 : %3\",\"{n}\",\"0:00:00\",\"Forced subtitles track #{n} turned out to be empty and was removed from output file\"\n"
        )
    }

    #[test]
    fn read_errors_count_the_lines_at_each_offset_in_order_of_first_appearance() {
        let medium = "Scsi error - MEDIUM ERROR:L-EC UNCORRECTABLE ERROR";
        let log = [
            read_error_line(SCRAMBLED, "1048576"),
            removal(5),
            read_error_line(SCRAMBLED, "1048576"),
            read_error_line(medium, "2097152"),
            read_error_line(SCRAMBLED, "1048576"),
        ]
        .concat();
        assert_eq!(
            read_errors(&log),
            vec![
                ReadError { offset: "1048576".to_string(), count: 3, message: SCRAMBLED.to_string() },
                ReadError { offset: "2097152".to_string(), count: 1, message: medium.to_string() },
            ]
        );
    }

    #[test]
    fn a_read_error_keeps_the_first_error_text_at_its_offset_and_never_the_drive_name() {
        let log = [
            read_error_line(SCRAMBLED, "1048576"),
            read_error_line("Scsi error - HARDWARE ERROR:TRACKING SERVO FAILURE", "1048576"),
        ]
        .concat();
        let errors = read_errors(&log);
        assert_eq!(errors, vec![ReadError { offset: "1048576".to_string(), count: 2, message: SCRAMBLED.to_string() }]);
        let shown = serde_json::to_string(&errors).unwrap();
        assert_eq!(shown, format!(r#"[{{"offset":"1048576","count":2,"message":"{SCRAMBLED}"}}]"#));
        assert!(!shown.contains("EXAMPLE DRIVE") && !shown.contains("SERIAL0001"));
    }

    #[test]
    fn a_read_error_line_without_its_parameters_is_still_counted() {
        let log = "MSG:2003,0,0,\"Read error\",\"Read error\"\n";
        assert_eq!(read_errors(log), vec![ReadError { offset: String::new(), count: 1, message: String::new() }]);
    }

    #[test]
    fn a_rip_log_without_msg_2003_has_no_read_errors() {
        // MSG:2023 counts read errors of one type (9.4: seen, not acted on); its wording here is made up.
        let log = [
            removal(5),
            "MSG:2023,0,1,\"Found 3 read errors\",\"Found %1 read errors\",\"3\"\n".to_string(),
            "MSG:5036,260,1,\"Copy complete. 1 titles saved.\",\"Copy complete. %1 titles saved.\",\"1\"\n".to_string(),
        ]
        .concat();
        assert_eq!(read_errors(&log), Vec::<ReadError>::new());
        assert_eq!(read_errors(""), Vec::<ReadError>::new());
    }

    /// One track of `mkvmerge -J` with these properties.
    fn track(id: i64, kind: &str, codec: &str, properties: Value) -> Value {
        json!({"id": id, "type": kind, "codec": codec, "properties": properties})
    }

    /// `mkvmerge -J`'s answer: the tracks, one chapter edition per entry of `chapters`, the duration in nanoseconds.
    fn file(tracks: Vec<Value>, chapters: &[i64], duration_ns: i64) -> MkvmergeIdent {
        let editions: Vec<Value> = chapters.iter().map(|entries| json!({"num_entries": entries})).collect();
        serde_json::from_value(json!({
            "tracks": tracks,
            "chapters": editions,
            "container": {"properties": {"duration": duration_ns}}
        }))
        .unwrap()
    }

    /// pans-labyrinth-2006-uhd's all-tracks file: the scan's streams 0 to 4 and 6 (MakeMKV removed 5 and 7).
    fn bluray_tracks() -> Vec<Value> {
        vec![
            track(0, "video", "HEVC/H.265/MPEG-H", json!({"language": "eng"})),
            track(1, "audio", "DTS-HD Master Audio", json!({"language": "spa", "audio_channels": 6})),
            track(2, "audio", "DTS", json!({"language": "spa", "audio_channels": 6})),
            track(3, "audio", "AC-3", json!({"language": "eng", "audio_channels": 2})),
            track(4, "subtitles", "HDMV PGS", json!({"language": "eng"})),
            track(5, "subtitles", "HDMV PGS", json!({"language": "spa"})),
        ]
    }

    /// The bluray fixture's file duration, 7150.1 s: the figure phase 0's EXP-01 publishes for pans-labyrinth-2006-uhd's
    /// file (`docs/phase0/experiments/EXP-01-profile-and-order.md`), not the clone's millisecond value (RP21).
    const BLURAY_NS: i64 = 7_150_100_000_000;

    /// The all-tracks rip log's two removals: "#5" and "#7".
    fn bluray_rip() -> String {
        [removal(5), removal(7)].concat()
    }

    /// The eng-only file of the bluray fixture: video, English Dolby Digital, English PGS.
    fn bluray_eng_tracks() -> Vec<Value> {
        vec![
            track(0, "video", "HEVC/H.265/MPEG-H", json!({"language": "eng"})),
            track(1, "audio", "AC-3", json!({"language": "eng", "audio_channels": 2})),
            track(2, "subtitles", "HDMV PGS", json!({"language": "eng"})),
        ]
    }

    /// short-circuit-2-1988-dvd's eng-only file: video, English Dolby Digital, the text-converted captions, then the
    /// English VobSub.
    fn dvd_eng_tracks() -> Vec<Value> {
        vec![
            track(0, "video", "MPEG-1/2", json!({"language": "eng"})),
            track(1, "audio", "AC-3", json!({"language": "eng", "audio_channels": 2})),
            track(2, "subtitles", "SubRip/SRT", json!({"language": "eng"})),
            track(3, "subtitles", "VobSub", json!({"language": "eng"})),
        ]
    }

    /// The dvd fixture's file duration, 6623.6 s: spec 6.2 step 7's figure for short-circuit-2-1988-dvd's file, not the
    /// clone's millisecond value (RP21).
    const DVD_NS: i64 = 6_623_600_000_000;

    /// The bluray fixture's all-tracks rip of `tracks`, with its rip log.
    fn bluray_report(tracks: Vec<Value>) -> VerifyReport {
        let rip = bluray_rip();
        verify_rip(&title_of(&bluray_scan(), 0), &file(tracks, &[22], BLURAY_NS), &[0, 1, 2, 3, 4, 5, 6, 7], Some(rip.as_str()), false)
    }

    #[test]
    fn a_blu_ray_rip_that_matches_the_scan_verifies() {
        assert_eq!(
            bluray_report(bluray_tracks()),
            VerifyReport {
                duration_ok: true,
                expected_seconds: 7150.0,
                file_seconds: 7150.1,
                chapters_ok: true,
                scan_chapters: 22,
                file_chapters: 22,
                tracks_ok: true,
                pairs: vec![(0, Some(0)), (1, Some(1)), (2, Some(2)), (3, Some(3)), (4, Some(4)), (6, Some(5))],
                extra_tracks: vec![],
                unknown_removed: vec![],
                mismatches: vec![],
                unchecked: vec![],
                read_errors: vec![],
                ok: true,
            }
        );
    }

    #[test]
    fn an_eng_rip_verifies_with_the_removal_counted_over_its_selection() {
        let rip = removal(3);
        let title = title_of(&bluray_scan(), 0);
        let report = verify_rip(&title, &file(bluray_eng_tracks(), &[22], BLURAY_NS), &[0, 3, 4, 5], Some(rip.as_str()), false);
        assert!(report.ok, "{:?}", report.mismatches);
        assert_eq!(report.pairs, vec![(0, Some(0)), (3, Some(1)), (4, Some(2))]);
        assert_eq!(report.unknown_removed, Vec::<i64>::new());
    }

    #[test]
    fn a_dvd_rip_verifies_with_its_caption_track_after_the_audio() {
        let title = title_of(&dvd_scan(), 0);
        let report = verify_rip(&title, &file(dvd_eng_tracks(), &[28], DVD_NS), &[0, 1, 3, 5], None, true);
        assert!(report.ok, "{:?}", report.mismatches);
        assert!(report.duration_ok);
        assert_eq!(report.pairs, vec![(0, Some(0)), (1, Some(1)), (5, Some(2)), (3, Some(3))]);
        assert_eq!((report.expected_seconds, report.file_seconds), (6606.0, 6623.6));
        assert_eq!((report.scan_chapters, report.file_chapters), (28, 28));
    }

    #[test]
    fn the_same_dvd_file_judged_as_a_blu_ray_fails_only_on_duration() {
        let title = title_of(&dvd_scan(), 0);
        let report = verify_rip(&title, &file(dvd_eng_tracks(), &[28], DVD_NS), &[0, 1, 3, 5], None, false);
        assert_eq!(report.mismatches, ["duration: expected 6606 s, file 6623.6 s"]);
        assert!(!report.duration_ok && report.tracks_ok && report.chapters_ok && !report.ok);
    }

    #[test]
    fn a_caption_track_in_scan_order_fails_the_ordered_match() {
        let mut tracks = dvd_eng_tracks();
        tracks[2]["codec"] = json!("VobSub");
        tracks[3]["codec"] = json!("SubRip/SRT");
        let report = verify_rip(&title_of(&dvd_scan(), 0), &file(tracks, &[28], DVD_NS), &[0, 1, 3, 5], None, true);
        assert_eq!(
            report.mismatches,
            ["track 2: codec expected SubRip/SRT, file VobSub", "track 3: codec expected VobSub, file SubRip/SRT"]
        );
        assert!(!report.tracks_ok && !report.ok);
    }

    #[test]
    fn a_wrong_codec_is_a_mismatch_naming_both() {
        let mut tracks = bluray_tracks();
        tracks[1]["codec"] = json!("AC-3");
        let report = bluray_report(tracks);
        assert_eq!(report.mismatches, ["track 1: codec expected DTS-HD Master Audio, file AC-3"]);
        assert!(!report.tracks_ok && !report.ok);
    }

    #[test]
    fn both_truehd_names_verify_and_another_codec_does_not() {
        let title = title_of(&truehd_scan(), 0);
        let run = |name: &str| {
            let tracks = vec![
                track(0, "video", "AVC/H.264/MPEG-4p10", json!({"language": "eng"})),
                track(1, "audio", name, json!({"language": "eng", "audio_channels": 8})),
                track(2, "audio", "TrueHD", json!({"language": "eng", "audio_channels": 6})),
            ];
            verify_rip(&title, &file(tracks, &[2], 600_000_000_000), &[0, 1, 2], None, false)
        };
        assert!(run("TrueHD Atmos").ok);
        assert!(run("TrueHD").ok);
        assert_eq!(run("AC-3").mismatches, ["track 1: codec expected TrueHD or TrueHD Atmos, file AC-3"]);
    }

    #[test]
    fn languages_compare_through_norm_lang_and_never_on_video() {
        let title = title_of(&dvd_scan(), 0);
        let tracks = vec![
            track(0, "video", "MPEG-1/2", json!({"language": "und"})),
            track(1, "audio", "AC-3", json!({"language": "eng", "audio_channels": 2})),
            track(2, "audio", "AC-3", json!({"language": "fra", "audio_channels": 2})),
            track(3, "subtitles", "SubRip/SRT", json!({"language": "eng"})),
            track(4, "subtitles", "VobSub", json!({"language": "eng"})),
            track(5, "subtitles", "VobSub", json!({"language": "fre"})),
        ];
        let run = |tracks: Vec<Value>| verify_rip(&title, &file(tracks, &[28], DVD_NS), &[0, 1, 2, 3, 4, 5], None, true);
        // The scan's "fre" and the file's "fra" are one language; the video's "und" is never compared.
        assert!(run(tracks.clone()).ok);
        let mut spanish = tracks.clone();
        spanish[2]["properties"]["language"] = json!("spa");
        assert_eq!(run(spanish).mismatches, ["track 2: language expected fra, file spa"]);
        let mut unlabelled = tracks;
        unlabelled[5]["properties"] = json!({});
        assert_eq!(run(unlabelled).mismatches, ["track 5: language expected fra, file und"]);
    }

    #[test]
    fn a_wrong_or_missing_channel_count_is_a_mismatch() {
        let mut six = bluray_tracks();
        six[3]["properties"]["audio_channels"] = json!(6);
        assert_eq!(bluray_report(six).mismatches, ["track 3: channels expected 2, file 6"]);
        let mut none = bluray_tracks();
        none[3]["properties"] = json!({"language": "eng"});
        assert_eq!(bluray_report(none).mismatches, ["track 3: channels expected 2, file none"]);
    }

    #[test]
    fn a_track_of_the_wrong_type_is_reported_by_its_type_alone() {
        let mut tracks = bluray_tracks();
        tracks[4]["type"] = json!("audio");
        assert_eq!(bluray_report(tracks).mismatches, ["track 4: type expected subtitles, file audio"]);
    }

    #[test]
    fn a_missing_track_is_paired_with_none_and_named_by_its_stream() {
        let mut tracks = bluray_tracks();
        tracks.pop();
        let report = bluray_report(tracks);
        assert_eq!(report.pairs.last(), Some(&(6, None)));
        assert_eq!(report.mismatches, ["stream 6 (subtitles): missing from the file"]);
        assert_eq!(report.extra_tracks, Vec::<i64>::new());
        assert!(!report.tracks_ok && !report.ok);
    }

    #[test]
    fn an_extra_file_track_is_listed_and_named() {
        let mut tracks = bluray_tracks();
        tracks.push(track(6, "subtitles", "HDMV PGS", json!({"language": "eng"})));
        let report = bluray_report(tracks);
        assert_eq!(report.extra_tracks, [6]);
        assert_eq!(report.mismatches, ["track 6: not expected (subtitles HDMV PGS)"]);
        assert!(!report.tracks_ok && !report.ok);
    }

    #[test]
    fn removal_indices_beyond_the_selection_are_listed_and_do_not_fail() {
        let rip = [removal(3), removal(9)].concat();
        let title = title_of(&bluray_scan(), 0);
        let report = verify_rip(&title, &file(bluray_eng_tracks(), &[22], BLURAY_NS), &[0, 3, 4, 5], Some(rip.as_str()), false);
        assert_eq!(report.unknown_removed, [9]);
        assert!(report.ok, "{:?}", report.mismatches);
    }

    #[test]
    fn without_a_rip_log_no_track_is_removed() {
        let title = title_of(&bluray_scan(), 0);
        let report = verify_rip(&title, &file(bluray_tracks(), &[22], BLURAY_NS), &[0, 1, 2, 3, 4, 5, 6, 7], None, false);
        assert_eq!(
            report.pairs,
            vec![(0, Some(0)), (1, Some(1)), (2, Some(2)), (3, Some(3)), (4, Some(4)), (5, Some(5)), (6, None), (7, None)]
        );
        assert_eq!(
            report.mismatches,
            [
                "track 5: language expected eng, file spa",
                "stream 6 (subtitles): missing from the file",
                "stream 7 (subtitles): missing from the file",
            ]
        );
        assert_eq!(report.read_errors, Vec::<ReadError>::new());
        assert!(!report.ok);
    }

    #[test]
    fn a_title_without_a_duration_fails_with_its_reason() {
        let scan = parse(&scan_text(
            &[],
            &[(
                0,
                &[(8, "2")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")]),
                    (1, &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")]),
                ],
            )],
        ));
        let tracks = vec![
            track(0, "video", "AVC/H.264/MPEG-4p10", json!({"language": "eng"})),
            track(1, "audio", "AC-3", json!({"language": "eng", "audio_channels": 2})),
        ];
        let report = verify_rip(&title_of(&scan, 0), &file(tracks, &[2], 600_000_000_000), &[0, 1], None, false);
        assert_eq!(report.mismatches, ["duration: the scan gives no duration (attribute 9)"]);
        assert_eq!(report.expected_seconds, 0.0);
        assert!(!report.duration_ok && report.tracks_ok && !report.ok);
    }

    #[test]
    fn the_chapter_count_is_read_from_the_first_edition() {
        let title = title_of(&bluray_scan(), 0);
        let rip = bluray_rip();
        let run = |chapters: &[i64]| {
            verify_rip(&title, &file(bluray_tracks(), chapters, BLURAY_NS), &[0, 1, 2, 3, 4, 5, 6, 7], Some(rip.as_str()), false)
        };
        assert!(run(&[22]).chapters_ok);
        let short = run(&[21]);
        assert_eq!((short.chapters_ok, short.file_chapters), (false, 21));
        assert_eq!(short.mismatches, ["chapters: expected 22, file 21"]);
        let none = run(&[]);
        assert_eq!(none.mismatches, ["chapters: expected 22, file 0"]);
        let two = run(&[22, 5]);
        assert!(two.ok, "{:?}", two.mismatches);
        assert_eq!(two.file_chapters, 22);
    }

    #[test]
    fn an_empty_prediction_never_verifies() {
        let scan = parse(&scan_text(&[], &[(0, &[(8, "0"), (9, "0:10:00")], &[])]));
        let report = verify_rip(&title_of(&scan, 0), &file(Vec::new(), &[], 600_000_000_000), &[], None, false);
        assert_eq!(report.mismatches, ["tracks: the scan predicts no track for this title"]);
        assert!(report.duration_ok && report.chapters_ok && !report.tracks_ok && !report.ok);
    }

    #[test]
    fn a_rip_with_read_errors_still_verifies_and_carries_them() {
        let rip = [bluray_rip(), read_error_line(SCRAMBLED, "1048576"), read_error_line(SCRAMBLED, "1048576")].concat();
        let title = title_of(&bluray_scan(), 0);
        let report = verify_rip(&title, &file(bluray_tracks(), &[22], BLURAY_NS), &[0, 1, 2, 3, 4, 5, 6, 7], Some(rip.as_str()), false);
        assert!(report.ok, "{:?}", report.mismatches);
        assert_eq!(report.read_errors, vec![ReadError { offset: "1048576".to_string(), count: 2, message: SCRAMBLED.to_string() }]);
    }

    #[test]
    fn the_report_serialises_with_its_fields_in_order() {
        let report = bluray_report(bluray_tracks());
        let value = serde_json::to_value(&report).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "duration_ok",
                "expected_seconds",
                "file_seconds",
                "chapters_ok",
                "scan_chapters",
                "file_chapters",
                "tracks_ok",
                "pairs",
                "extra_tracks",
                "unknown_removed",
                "mismatches",
                "unchecked",
                "read_errors",
                "ok",
            ]
        );
        assert_eq!(value["pairs"][5], json!([6, 5]));
        assert_eq!(serde_json::from_value::<VerifyReport>(value).unwrap(), report);
    }

    /// A report that is `ok` or not, with these read errors; the other fields do not matter to failure_reviews.
    fn outcome(ok: bool, read_errors: Vec<ReadError>) -> VerifyReport {
        VerifyReport {
            duration_ok: ok,
            expected_seconds: 600.0,
            file_seconds: 600.0,
            chapters_ok: true,
            scan_chapters: 2,
            file_chapters: 2,
            tracks_ok: true,
            pairs: Vec::new(),
            extra_tracks: Vec::new(),
            unknown_removed: Vec::new(),
            mismatches: Vec::new(),
            unchecked: Vec::new(),
            read_errors,
            ok,
        }
    }

    /// short-circuit-2-1988-dvd's read error: three lines at offset 1048576.
    fn scrambled() -> ReadError {
        ReadError { offset: "1048576".to_string(), count: 3, message: SCRAMBLED.to_string() }
    }

    #[test]
    fn verify_failed_is_raised_only_by_a_second_failure() {
        assert_eq!(failure_reviews(&outcome(false, Vec::new()), false), Vec::<ReviewReason>::new());
        assert_eq!(failure_reviews(&outcome(false, Vec::new()), true), [ReviewReason::VerifyFailed]);
        assert_eq!(failure_reviews(&outcome(true, Vec::new()), true), Vec::<ReviewReason>::new());
    }

    #[test]
    fn read_errors_are_a_review_reason_whether_or_not_the_rip_verified() {
        assert_eq!(failure_reviews(&outcome(true, vec![scrambled()]), false), [ReviewReason::ReadErrors]);
        assert_eq!(failure_reviews(&outcome(false, vec![scrambled()]), false), [ReviewReason::ReadErrors]);
    }

    #[test]
    fn both_reasons_come_in_the_specs_order() {
        assert_eq!(
            failure_reviews(&outcome(false, vec![scrambled()]), true),
            [ReviewReason::VerifyFailed, ReviewReason::ReadErrors]
        );
    }

    /// Title 0 of a Blu-ray scan: AVC video and one English audio stream of these attributes 5, 6 and 7 and channels.
    fn audio_scan(codec: (&str, &str, &str), channels: &str) -> Scan {
        parse(&scan_text(
            &[(1, "Blu-ray disc")],
            &[(
                0,
                &[(8, "1"), (9, "0:10:00")],
                &[
                    (0, &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")]),
                    (1, &[(1, "Audio"), (3, "eng"), (5, codec.0), (6, codec.1), (7, codec.2), (14, channels)]),
                ],
            )],
        ))
    }

    /// The file of audio_scan: AVC video, then an audio track named `codec` with this language and channel count.
    fn audio_file(codec: &str, language: &str, channels: i64) -> MkvmergeIdent {
        let tracks = vec![
            track(0, "video", "AVC/H.264/MPEG-4p10", json!({"language": "eng"})),
            track(1, "audio", codec, json!({"language": language, "audio_channels": channels})),
        ];
        file(tracks, &[1], 600_000_000_000)
    }

    #[test]
    fn dts_es_dts_96_24_and_dts_express_rips_verify_under_mkvmerges_names() {
        // Not seen in a recording: mkvmerge's documented names for a DTS-ES 6.1 and a DTS 96/24 stream (MakeMKV's DTS
        // family) and for a DTS Express picture-in-picture commentary (A_DTS, attribute 6 "DTS-HD LBR").
        for (codec, channels, written) in [
            (("A_DTS", "DTS", "DTS"), "7", "DTS-ES"),
            (("A_DTS", "DTS", "DTS"), "6", "DTS 96/24"),
            (("A_DTS", "DTS-HD LBR", "DTS Express"), "2", "DTS Express"),
        ] {
            let title = title_of(&audio_scan(codec, channels), 0);
            let file = audio_file(written, "eng", channels.parse().unwrap());
            let report = verify_rip(&title, &file, &[0, 1], None, false);
            assert_eq!(report.mismatches, Vec::<String>::new(), "{written}");
            assert!(report.ok, "{written}");
            assert_eq!(failure_reviews(&report, true), Vec::<ReviewReason>::new(), "{written}");
        }
    }

    #[test]
    fn a_codec_the_table_does_not_know_is_not_compared_and_its_rip_verifies() {
        // Not seen in a recording: an audio stream whose codec the table does not know (a made-up A_OPUS, Family
        // Unknown), beside AVC video. Its codec name is not compared and the report says so; its type, language and
        // channels are, so a correct rip is neither ripped again nor held with verify_failed.
        let title = title_of(&audio_scan(("A_OPUS", "Opus", "Opus"), "2"), 0);
        let expected = expected_tracks(&title, &[0, 1], &BTreeSet::new());
        assert_eq!(expected.len(), 2);
        assert_eq!(expected[0].codecs, ["AVC/H.264/MPEG-4p10"]);
        assert!(expected[1].codecs.is_empty(), "{expected:?}");
        let report = verify_rip(&title, &audio_file("Opus", "eng", 2), &[0, 1], None, false);
        assert_eq!(report.mismatches, Vec::<String>::new());
        assert_eq!(
            report.unchecked,
            ["track 1: codec not compared, the codec table does not know the scan's codec (file Opus)"]
        );
        assert!(report.tracks_ok && report.ok);
        assert_eq!(failure_reviews(&report, true), Vec::<ReviewReason>::new());
        let wrong = verify_rip(&title, &audio_file("Opus", "spa", 6), &[0, 1], None, false);
        assert_eq!(wrong.mismatches, ["track 1: language expected eng, file spa", "track 1: channels expected 2, file 6"]);
        assert!(!wrong.ok);
        assert_eq!(failure_reviews(&wrong, true), [ReviewReason::VerifyFailed]);
        // VC-1 video, by contrast, is a known family (Task 1's Vc1, ruling RP26; not seen in a recording): it expects
        // mkvmerge's documented name "VC-1", also not yet seen, and a file writing that name verifies.
        let vc1 = parse(&scan_text(
            &[(1, "Blu-ray disc")],
            &[(0, &[(8, "1"), (9, "0:10:00")], &[(0, &[(1, "Video"), (5, "V_MS/VFW/WVC1"), (6, "VC-1"), (7, "VC-1 Advanced@L3")])])],
        ));
        assert_eq!(expected_tracks(&title_of(&vc1, 0), &[0], &BTreeSet::new())[0].codecs, ["VC-1"]);
        let vc1_file = file(vec![track(0, "video", "VC-1", json!({}))], &[1], 600_000_000_000);
        let vc1_report = verify_rip(&title_of(&vc1, 0), &vc1_file, &[0], None, false);
        assert_eq!(vc1_report.mismatches, Vec::<String>::new());
        assert!(vc1_report.ok);
        assert_eq!(failure_reviews(&vc1_report, true), Vec::<ReviewReason>::new());
    }

    /// An output track with flags [default, forced, hearing_impaired, original].
    fn output(stream: u32, kind: StreamKind, language: Option<&str>, name: Option<&str>, flags: [bool; 4]) -> OutputTrack {
        OutputTrack {
            stream,
            kind,
            language: language.map(str::to_string),
            name: name.map(str::to_string),
            default: flags[0],
            forced: flags[1],
            hearing_impaired: flags[2],
            original: flags[3],
        }
    }

    /// The layout of a Spanish film (8.10), as Task 11's `layout` writes it: video, default (the first video track always
    /// is); Spanish DTS-HD MA 5.1, default and original; English Dolby Digital 2.0; English subtitles, default (the default
    /// audio is not English); English SDH subtitles.
    fn spanish_layout() -> Layout {
        Layout {
            tracks: vec![
                output(0, StreamKind::Video, None, None, [true, false, false, false]),
                output(1, StreamKind::Audio, Some("spa"), Some("DTS-HD MA 5.1"), [true, false, false, true]),
                output(3, StreamKind::Audio, Some("eng"), Some("Dolby Digital 2.0"), [false, false, false, false]),
                output(4, StreamKind::Subtitles, Some("eng"), Some("English"), [true, false, false, false]),
                output(7, StreamKind::Subtitles, Some("eng"), Some("English (SDH)"), [false, false, true, false]),
            ],
            chapters_kept: true,
        }
    }

    /// The partial file mkvmerge wrote for spanish_layout.
    fn spanish_tracks() -> Vec<Value> {
        vec![
            track(0, "video", "HEVC/H.265/MPEG-H", json!({"language": "und", "default_track": true})),
            track(
                1,
                "audio",
                "DTS-HD Master Audio",
                json!({"language": "spa", "track_name": "DTS-HD MA 5.1", "default_track": true, "forced_track": false, "flag_original": true, "audio_channels": 6}),
            ),
            track(2, "audio", "AC-3", json!({"language": "eng", "track_name": "Dolby Digital 2.0", "default_track": false, "audio_channels": 2})),
            track(3, "subtitles", "HDMV PGS", json!({"language": "eng", "track_name": "English", "default_track": true})),
            track(4, "subtitles", "HDMV PGS", json!({"language": "eng", "track_name": "English (SDH)", "default_track": false, "flag_hearing_impaired": true})),
        ]
    }

    /// A two-track layout with every flag false and no names: video and English audio.
    fn plain_layout() -> Layout {
        Layout {
            tracks: vec![
                output(0, StreamKind::Video, None, None, [false, false, false, false]),
                output(1, StreamKind::Audio, Some("eng"), None, [false, false, false, false]),
            ],
            chapters_kept: true,
        }
    }

    /// The scan title of spanish_layout: the bluray fixture's title 0.
    fn spanish_title() -> Title {
        title_of(&bluray_scan(), 0)
    }

    /// The scan title of plain_layout: AVC video and English Dolby Digital 2.0.
    fn plain_title() -> Title {
        title_of(&audio_scan(("A_AC3", "DD", "Dolby Digital"), "2"), 0)
    }

    /// The staged file's report as verify_final reads it: a verified rip of this duration and chapter count.
    fn staged(file_seconds: f64, file_chapters: u32) -> VerifyReport {
        VerifyReport { file_seconds, file_chapters, ..outcome(true, Vec::new()) }
    }

    /// The file for plain_layout, without any flag property.
    fn plain_tracks() -> Vec<Value> {
        vec![track(0, "video", "AVC/H.264/MPEG-4p10", json!({})), track(1, "audio", "AC-3", json!({"language": "eng", "audio_channels": 2}))]
    }

    #[test]
    fn a_remuxed_file_that_matches_the_layout_verifies() {
        let report = verify_final(&spanish_layout(), &spanish_title(), &staged(7150.1, 22), &file(spanish_tracks(), &[22], BLURAY_NS));
        assert_eq!(
            report,
            VerifyReport {
                duration_ok: true,
                expected_seconds: 7150.1,
                file_seconds: 7150.1,
                chapters_ok: true,
                scan_chapters: 22,
                file_chapters: 22,
                tracks_ok: true,
                pairs: vec![(0, Some(0)), (1, Some(1)), (3, Some(2)), (4, Some(3)), (7, Some(4))],
                extra_tracks: vec![],
                unknown_removed: vec![],
                mismatches: vec![],
                unchecked: vec![],
                read_errors: vec![],
                ok: true,
            }
        );
    }

    #[test]
    fn every_flag_difference_is_a_sentence() {
        let mut tracks = spanish_tracks();
        tracks[1]["properties"]["default_track"] = json!(false);
        tracks[1]["properties"]["flag_original"] = json!(false);
        tracks[3]["properties"]["forced_track"] = json!(true);
        tracks[4]["properties"]["flag_hearing_impaired"] = json!(false);
        let report = verify_final(&spanish_layout(), &spanish_title(), &staged(7150.1, 22), &file(tracks, &[22], BLURAY_NS));
        assert_eq!(
            report.mismatches,
            [
                "track 1: default expected true, file false",
                "track 1: original expected true, file false",
                "track 3: forced expected false, file true",
                "track 4: hearing_impaired expected true, file false",
            ]
        );
        assert!(!report.tracks_ok && !report.ok);
    }

    #[test]
    fn names_and_languages_are_compared_and_an_absent_name_is_none() {
        let mut tracks = spanish_tracks();
        tracks[2]["properties"]["language"] = json!("spa");
        tracks[2]["properties"]["track_name"] = json!("Stereo");
        tracks[3]["properties"] = json!({"language": "eng", "default_track": true});
        let report = verify_final(&spanish_layout(), &spanish_title(), &staged(7150.1, 22), &file(tracks, &[22], BLURAY_NS));
        assert_eq!(
            report.mismatches,
            [
                "track 2: language expected eng, file spa",
                "track 2: name expected Dolby Digital 2.0, file Stereo",
                "track 3: name expected English, file none",
            ]
        );
    }

    #[test]
    fn an_absent_flag_property_counts_as_false() {
        let report = verify_final(&plain_layout(), &plain_title(), &staged(600.0, 0), &file(plain_tracks(), &[], 600_000_000_000));
        assert!(report.ok, "{:?}", report.mismatches);
        assert_eq!(report.pairs, vec![(0, Some(0)), (1, Some(1))]);
    }

    #[test]
    fn the_final_duration_has_the_two_second_band_only() {
        let run = |file_ns: i64, source: f64| {
            verify_final(&plain_layout(), &plain_title(), &staged(source, 0), &file(plain_tracks(), &[], file_ns))
        };
        assert!(run(6_625_500_000_000, 6623.6).ok);
        let long = run(6_626_000_000_000, 6623.6);
        assert_eq!(long.mismatches, ["duration: expected 6623.6 s, file 6626.0 s"]);
        assert!(!long.duration_ok && !long.ok);
        // The DVD band of 6.2 step 7 does not apply: the source is the staged file, not the scan.
        assert!(!run(DVD_NS, 6606.0).duration_ok);
    }

    #[test]
    fn a_missing_an_extra_and_a_wrong_type_track_fail_the_final_check() {
        let run = |tracks: Vec<Value>| {
            verify_final(&plain_layout(), &plain_title(), &staged(600.0, 0), &file(tracks, &[], 600_000_000_000))
        };
        let missing = run(vec![track(0, "video", "AVC/H.264/MPEG-4p10", json!({}))]);
        assert_eq!(missing.mismatches, ["stream 1 (audio): missing from the file"]);
        assert_eq!(missing.pairs, vec![(0, Some(0)), (1, None)]);
        let mut extra_tracks = plain_tracks();
        extra_tracks.push(track(2, "subtitles", "HDMV PGS", json!({"language": "eng"})));
        let extra = run(extra_tracks);
        assert_eq!(extra.mismatches, ["track 2: not expected (subtitles HDMV PGS)"]);
        assert_eq!(extra.extra_tracks, [2]);
        let mut wrong = plain_tracks();
        wrong[1]["type"] = json!("subtitles");
        assert_eq!(run(wrong).mismatches, ["track 1: type expected audio, file subtitles"]);
    }

    #[test]
    fn the_video_language_is_not_compared_in_the_final_file() {
        let mut layout = plain_layout();
        layout.tracks[0].language = Some("jpn".to_string());
        let report = verify_final(&layout, &plain_title(), &staged(600.0, 0), &file(plain_tracks(), &[], 600_000_000_000));
        assert!(report.ok, "{:?}", report.mismatches);
    }

    #[test]
    fn an_empty_layout_never_verifies() {
        let empty = Layout { tracks: vec![], chapters_kept: true };
        let report = verify_final(&empty, &plain_title(), &staged(600.0, 0), &file(Vec::new(), &[], 600_000_000_000));
        assert_eq!(report.mismatches, ["tracks: the layout keeps no track"]);
        assert!(report.duration_ok && !report.tracks_ok && !report.ok);
    }

    #[test]
    fn a_remuxed_track_whose_codec_or_channels_are_not_its_source_streams_fails() {
        // The remux mapped layout stream 3 (English Dolby Digital 2.0) to the Spanish DTS core's raw track: the language,
        // name and flags are the layout's, the codec and the channels are not those of stream 3.
        let mut tracks = spanish_tracks();
        tracks[2]["codec"] = json!("DTS");
        tracks[2]["properties"]["audio_channels"] = json!(6);
        let report = verify_final(&spanish_layout(), &spanish_title(), &staged(7150.1, 22), &file(tracks, &[22], BLURAY_NS));
        assert_eq!(report.mismatches, ["track 2: codec expected AC-3, file DTS", "track 2: channels expected 2, file 6"]);
        assert!(!report.tracks_ok && !report.ok);
        // A layout stream the title does not hold is a difference of its own.
        let mut layout = plain_layout();
        layout.tracks[1].stream = 9;
        let report = verify_final(&layout, &plain_title(), &staged(600.0, 0), &file(plain_tracks(), &[], 600_000_000_000));
        assert_eq!(report.mismatches, ["track 1: stream 9 is not a stream of the title"]);
        assert!(!report.ok);
    }

    #[test]
    fn a_remux_keeps_the_staged_files_chapter_count_when_the_layout_keeps_chapters() {
        let run = |layout: &Layout, chapters: &[i64]| {
            verify_final(layout, &spanish_title(), &staged(7150.1, 22), &file(spanish_tracks(), chapters, BLURAY_NS))
        };
        let dropped = run(&spanish_layout(), &[]);
        assert_eq!(dropped.mismatches, ["chapters: expected 22, file 0"]);
        assert!(dropped.tracks_ok && dropped.duration_ok && !dropped.chapters_ok && !dropped.ok);
        assert_eq!((dropped.scan_chapters, dropped.file_chapters), (22, 0));
        assert_eq!(run(&spanish_layout(), &[21]).mismatches, ["chapters: expected 22, file 21"]);
        // A layout that keeps no chapters expects none.
        let unchaptered = Layout { chapters_kept: false, ..spanish_layout() };
        assert!(run(&unchaptered, &[]).ok);
        assert_eq!(run(&unchaptered, &[22]).mismatches, ["chapters: expected 0, file 22"]);
    }

    #[test]
    fn a_remuxed_track_of_a_codec_the_table_does_not_know_has_only_its_codec_left_uncompared() {
        let title = title_of(&audio_scan(("A_OPUS", "Opus", "Opus"), "2"), 0);
        let tracks = |channels: i64| {
            vec![
                track(0, "video", "AVC/H.264/MPEG-4p10", json!({})),
                track(1, "audio", "Opus", json!({"language": "eng", "audio_channels": channels})),
            ]
        };
        let report = verify_final(&plain_layout(), &title, &staged(600.0, 0), &file(tracks(2), &[], 600_000_000_000));
        assert_eq!(report.mismatches, Vec::<String>::new());
        assert_eq!(
            report.unchecked,
            ["track 1: codec not compared, the codec table does not know the scan's codec (file Opus)"]
        );
        assert!(report.ok);
        let six = verify_final(&plain_layout(), &title, &staged(600.0, 0), &file(tracks(6), &[], 600_000_000_000));
        assert_eq!(six.mismatches, ["track 1: channels expected 2, file 6"]);
    }

    /// The dvd fixture's eng-only rip, verified: pairs (0, 0), (1, 1), (5, 2), (3, 3).
    fn dvd_report() -> VerifyReport {
        verify_rip(&title_of(&dvd_scan(), 0), &file(dvd_eng_tracks(), &[28], DVD_NS), &[0, 1, 3, 5], None, true)
    }

    /// One subs/index.json entry of an English track.
    fn entry(n: i64, track_id: Option<i64>, codec: &str) -> SubsIndexEntry {
        SubsIndexEntry {
            n,
            track_id,
            language: Some("eng".to_string()),
            codec: Some(codec.to_string()),
            track_name: None,
            forced: Some(false),
            default: Some(false),
        }
    }

    /// subs/index.json for the dvd fixture's eng-only file: n 0 is track 2 (text), n 1 is track 3 (VobSub).
    const INDEX: &str = r#"[{"n": 0, "track_id": 2, "language": "eng", "codec": "SubRip/SRT", "track_name": null, "forced": false, "default": true}, {"n": 1, "track_id": 3, "language": "eng", "codec": "VobSub", "track_name": null, "forced": false, "default": false}]"#;

    /// The text track's packets, made up: three events of 20, 40 and 80 bytes (the two of 64 bytes or less still count,
    /// since a text packet never clears the screen).
    const TEXT_PACKETS: &str = r#"{"packets": [{"pts_time": "10.000000", "duration_time": "2.000000", "size": "20"}, {"pts_time": "12.500000", "duration_time": "2.000000", "size": "40"}, {"pts_time": "15.250000", "duration_time": "2.000000", "size": "80"}]}"#;

    /// The VobSub track's packets, made up: two of 64 bytes or less (screen clears), two larger.
    const IMAGE_PACKETS: &str = r#"{"packets": [{"pts_time": "20.000000", "duration_time": "0.500000", "size": "30"}, {"pts_time": "21.000000", "duration_time": "1.000000", "size": "1400"}, {"pts_time": "23.000000", "duration_time": "1.000000", "size": "3000"}, {"pts_time": "25.000000", "duration_time": "0.500000", "size": "64"}]}"#;

    /// A recording tree shown as "rec" holding these files.
    fn subs_tree(files: &[(&str, &str)]) -> MemTree {
        let mut tree = MemTree::new("rec", "rec");
        for (path, text) in files {
            tree.insert_file(path, *text);
        }
        tree
    }

    #[test]
    fn stream_of_track_follows_the_pairs() {
        let report = bluray_report(bluray_tracks());
        assert_eq!(stream_of_track(&report, 0), Some(0));
        assert_eq!(stream_of_track(&report, 4), Some(4));
        assert_eq!(stream_of_track(&report, 5), Some(6));
        assert_eq!(stream_of_track(&report, 9), None);
        let mut short = bluray_tracks();
        short.pop();
        // The match failed (stream 6 has no file track), so no track id stands for a stream.
        assert_eq!(stream_of_track(&bluray_report(short), 5), None);
    }

    #[test]
    fn a_report_whose_tracks_do_not_match_maps_no_file_track_to_a_stream() {
        let mut tracks = bluray_tracks();
        tracks[1]["codec"] = json!("AC-3");
        let report = bluray_report(tracks);
        assert!(!report.tracks_ok);
        assert_eq!(stream_of_track(&report, 0), None);
        assert_eq!(subs_streams(&[entry(0, Some(4), "HDMV PGS")], &report), BTreeMap::new());
    }

    #[test]
    fn subs_streams_map_index_entries_through_their_track_ids() {
        let report = bluray_report(bluray_tracks());
        let index = [
            entry(0, Some(4), "HDMV PGS"),
            entry(1, Some(5), "HDMV PGS"),
            entry(2, None, "HDMV PGS"),
            entry(3, Some(9), "HDMV PGS"),
        ];
        assert_eq!(subs_streams(&index, &report), BTreeMap::from([(0, 4), (1, 6)]));
        assert_eq!(subs_streams(&[entry(0, Some(2), "SubRip/SRT"), entry(1, Some(3), "VobSub")], &dvd_report()), BTreeMap::from([(0, 5), (1, 3)]));
    }

    #[test]
    fn by_scan_stream_rekeys_a_map_and_drops_unmapped_keys() {
        let by_n = BTreeMap::from([(0, "regular"), (1, "sdh"), (2, "forced")]);
        let streams = BTreeMap::from([(0, 4), (1, 6)]);
        assert_eq!(by_scan_stream(&by_n, &streams), BTreeMap::from([(4, "regular"), (6, "sdh")]));
    }

    #[test]
    fn subtitle_events_count_image_packets_above_the_default_clear_limit_and_every_text_packet() {
        let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        let default = Settings::default().sub_clear_max_bytes;
        assert_eq!(default, 64);
        assert_eq!(
            subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), default),
            Ok(BTreeMap::from([(3, vec![21.0, 23.0]), (5, vec![10.0, 12.5, 15.25])]))
        );
    }

    #[test]
    fn subtitle_events_at_a_clear_limit_of_zero_count_every_image_packet() {
        let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        assert_eq!(
            subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), 0),
            Ok(BTreeMap::from([(3, vec![20.0, 21.0, 23.0, 25.0]), (5, vec![10.0, 12.5, 15.25])]))
        );
    }

    #[test]
    fn subtitle_events_refuse_a_missing_or_unreadable_index() {
        let empty = subs_tree(&[("subs/sub-0.packets.json", TEXT_PACKETS)]);
        assert_eq!(
            subtitle_events(&empty, &PathKey::new("subs"), &dvd_report(), 64),
            Err("[Errno 2] No such file or directory: 'rec/subs/index.json'".to_string())
        );
        let broken = subs_tree(&[("subs/index.json", "not json")]);
        let error = subtitle_events(&broken, &PathKey::new("subs"), &dvd_report(), 64).unwrap_err();
        assert!(error.starts_with("rec/subs/index.json: "), "{error}");
    }

    #[test]
    fn subtitle_events_refuse_a_missing_packets_file_only_for_a_mapped_entry() {
        // n 1 names track 9, which no pair holds: its packets file is never read.
        let unmapped = r#"[{"n": 0, "track_id": 2, "codec": "SubRip/SRT"}, {"n": 1, "track_id": 9, "codec": "VobSub"}]"#;
        let tree = subs_tree(&[("subs/index.json", unmapped), ("subs/sub-0.packets.json", TEXT_PACKETS)]);
        assert_eq!(
            subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), 64),
            Ok(BTreeMap::from([(5, vec![10.0, 12.5, 15.25])]))
        );
        let mapped = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        assert_eq!(
            subtitle_events(&mapped, &PathKey::new("subs"), &dvd_report(), 64),
            Err("[Errno 2] No such file or directory: 'rec/subs/sub-0.packets.json'".to_string())
        );
    }

    #[test]
    fn subtitle_events_read_the_index_leniently_and_each_packets_file_strictly() {
        // index.json with a byte-order mark and an invalid byte in a track name: read as align::index_streams reads it.
        let index = b"\xef\xbb\xbf[{\"n\": 0, \"track_id\": 2, \"codec\": \"SubRip/SRT\", \"track_name\": \"CC\xff\"}]".to_vec();
        let mut tree = MemTree::new("rec", "rec");
        tree.insert_file("subs/index.json", index.clone());
        tree.insert_file("subs/sub-0.packets.json", TEXT_PACKETS);
        assert_eq!(
            subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), 64),
            Ok(BTreeMap::from([(5, vec![10.0, 12.5, 15.25])]))
        );
        // The same invalid byte in a packets file is refused, naming the file.
        let mut strict = MemTree::new("rec", "rec");
        strict.insert_file("subs/index.json", index);
        strict.insert_file("subs/sub-0.packets.json", b"{\xff}".to_vec());
        assert_eq!(
            subtitle_events(&strict, &PathKey::new("subs"), &dvd_report(), 64),
            Err("rec/subs/sub-0.packets.json: 'utf-8' codec can't decode byte 0xff in position 1: invalid start byte".to_string())
        );
    }

    /// The error of `subtitle_events` over the dvd fixture's index with `packets` as the text track's (n 0) file.
    fn text_packets_error(packets: &str) -> String {
        let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", packets), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), 64).unwrap_err()
    }

    #[test]
    fn subtitle_events_prefix_a_packets_file_that_is_not_an_object_with_its_path() {
        // For a top level that is not an object, load_packets returns Python's AttributeError text without the path;
        // subtitle_events adds it.
        assert_eq!(text_packets_error("[]"), "rec/subs/sub-0.packets.json: 'list' object has no attribute 'get'");
    }

    #[test]
    fn subtitle_events_never_prefix_a_path_twice() {
        // load_packets prefixes its own syntax and shape errors, so subtitle_events leaves those as they are.
        for packets in ["{", r#"{"packets": 5}"#] {
            let error = text_packets_error(packets);
            assert!(error.starts_with("rec/subs/sub-0.packets.json: "), "{packets}: {error}");
            assert_eq!(error.matches("rec/subs/sub-0.packets.json").count(), 1, "{packets}: {error}");
        }
    }

    #[test]
    fn subtitle_events_prefix_an_events_error_with_the_path() {
        assert_eq!(
            text_packets_error(r#"{"packets": [{"pts_time": "1.0", "size": "big"}]}"#),
            "rec/subs/sub-0.packets.json: invalid literal for int() with base 10: 'big'"
        );
        assert_eq!(
            text_packets_error(r#"{"packets": [{"pts_time": "soon", "size": "20"}]}"#),
            "rec/subs/sub-0.packets.json: could not convert string to float: 'soon'"
        );
        // A packet without a pts_time is skipped before its size is read, so a bad size on it is no error.
        let tree = subs_tree(&[
            ("subs/index.json", INDEX),
            ("subs/sub-0.packets.json", r#"{"packets": [{"size": "big"}, {"pts_time": "10.0", "size": "20"}]}"#),
            ("subs/sub-1.packets.json", IMAGE_PACKETS),
        ]);
        assert_eq!(
            subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), 64),
            Ok(BTreeMap::from([(3, vec![21.0, 23.0]), (5, vec![10.0])]))
        );
    }

    /// The packets files of `ns` under subs/ in `tree`, read as `subtitle_events` reads them.
    fn read_all(tree: &MemTree, ns: &[i64]) -> BTreeMap<i64, PacketsFile> {
        ns.iter().map(|&n| (n, read_packets_file(tree, &PathKey::new("subs"), n))).collect()
    }

    /// subs/index.json's entries.
    fn index_of(text: &str) -> Vec<SubsIndexEntry> {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn events_of_read_packets_equal_the_tree_form() {
        let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        let packets = read_all(&tree, &[0, 1]);
        assert_eq!(packets[&0].shown, "rec/subs/sub-0.packets.json");
        for limit in [64, 0] {
            assert_eq!(
                subtitle_events_of(&index_of(INDEX), &packets, &dvd_report(), limit),
                subtitle_events(&tree, &PathKey::new("subs"), &dvd_report(), limit)
            );
        }
        assert_eq!(
            subtitle_events_of(&index_of(INDEX), &packets, &dvd_report(), 64),
            Ok(BTreeMap::from([(3, vec![21.0, 23.0]), (5, vec![10.0, 12.5, 15.25])]))
        );
    }

    #[test]
    fn a_broken_unmapped_packets_file_is_silent() {
        // n 1 names track 9, which no pair holds.
        let unmapped = r#"[{"n": 0, "track_id": 2, "codec": "SubRip/SRT"}, {"n": 1, "track_id": 9, "codec": "VobSub"}]"#;
        let tree = subs_tree(&[("subs/index.json", unmapped), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", "[]")]);
        let packets = read_all(&tree, &[0, 1]);
        assert_eq!(
            packets[&1].packets.as_ref().err().map(String::as_str),
            Some("rec/subs/sub-1.packets.json: 'list' object has no attribute 'get'")
        );
        assert_eq!(
            subtitle_events_of(&index_of(unmapped), &packets, &dvd_report(), 64),
            Ok(BTreeMap::from([(5, vec![10.0, 12.5, 15.25])]))
        );
    }

    #[test]
    fn a_broken_mapped_packets_file_names_its_path() {
        let missing = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        assert_eq!(
            subtitle_events_of(&index_of(INDEX), &read_all(&missing, &[0, 1]), &dvd_report(), 64),
            Err("[Errno 2] No such file or directory: 'rec/subs/sub-0.packets.json'".to_string())
        );
        for (text, error) in [
            ("[]", "rec/subs/sub-0.packets.json: 'list' object has no attribute 'get'"),
            (
                r#"{"packets": [{"pts_time": "1.0", "size": "big"}]}"#,
                "rec/subs/sub-0.packets.json: invalid literal for int() with base 10: 'big'",
            ),
        ] {
            let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", text), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
            assert_eq!(
                subtitle_events_of(&index_of(INDEX), &read_all(&tree, &[0, 1]), &dvd_report(), 64),
                Err(error.to_string()),
                "{text}"
            );
        }
    }

    #[test]
    fn a_mapped_entry_whose_file_was_not_read_is_refused_naming_it() {
        let tree = subs_tree(&[("subs/index.json", INDEX), ("subs/sub-0.packets.json", TEXT_PACKETS), ("subs/sub-1.packets.json", IMAGE_PACKETS)]);
        assert_eq!(
            subtitle_events_of(&index_of(INDEX), &read_all(&tree, &[0]), &dvd_report(), 64),
            Err("sub-1.packets.json: not read".to_string())
        );
    }

    #[test]
    fn removed_scan_streams_counts_msg_4047_over_the_selection() {
        let selection = [0, 1, 2, 5, 6];
        // Output track #3 is the fourth selected stream: scan stream 5.
        assert_eq!(removed_scan_streams(Some(removal(3).as_str()), &selection), BTreeSet::from([5]));
        // Output track #9 is beyond the five selected streams: it names no stream.
        assert_eq!(removed_scan_streams(Some(removal(9).as_str()), &selection), BTreeSet::new());
        // Without a rip log nothing was removed.
        assert_eq!(removed_scan_streams(None, &selection), BTreeSet::new());
    }
}
