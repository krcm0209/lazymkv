//! The Matroska reader of evidence.mks (design part 10.3, [OCR-2]): the EBML (Extensible Binary Meta Language)
//! syntax of the subtitle-only file mkvmerge writes, read for one subtitle track's blocks with their content encoding
//! (K1: zlib, or header stripping) undone, and the VobSub idx header of its CodecPrivate. A pure function of bytes
//! (RP19).

use crate::subtitle::bitmap::{SubCodec, CODEC_PGS, CODEC_VOBSUB};

/// The VobSub idx header held in the track's CodecPrivate: the frame size, the video frame of trigger 4 and of the
/// golden canvas, and the 16-colour RGB (red, green, blue) palette (design part 10.3). Def: T5-3. Use: T5-4, T5-11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdxHeader {
    pub width: u32,
    pub height: u32,
    /// palette[i] = [R, G, B].
    pub palette: [[u8; 3]; 16],
}

/// One block of the stream: `pts` in seconds (cluster timestamp plus the block's relative timestamp, times the
/// segment's timestamp scale), which equals the packet's `pts_time` in `subs\sub-<n>.packets.json`; `payload` with
/// the content encoding undone and `size` its length, the size the cross-check compares (design parts 10.3, 10.12).
/// Def: T5-3. Use: T5-4, T5-6, T5-8, T5-11, T5-13.
#[derive(Clone, Debug, PartialEq)]
pub struct SubBlock {
    pub pts: f64,
    pub size: u64,
    pub payload: Vec<u8>,
}

/// The chosen subtitle track: its number, CodecID, codec (None for a codec other than PGS and VobSub, which the
/// caller refuses, T5-R11), VobSub idx header and blocks in file order (design part 10.3). Def: T5-3. Use: T5-8,
/// T5-11, T5-13.
#[derive(Clone, Debug, PartialEq)]
pub struct SubStream {
    pub track: u64,
    pub codec_id: String,
    pub codec: Option<SubCodec>,
    pub idx: Option<IdxHeader>,
    pub blocks: Vec<SubBlock>,
}

/// The refusal of bytes that do not follow the EBML rules of a Matroska file.
const NOT_MATROSKA: &str = "not a Matroska file";

const EBML: u32 = 0x1A45_DFA3;
const DOC_TYPE: u32 = 0x4282;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const CONTENT_ENCODINGS: u32 = 0x6D80;
const CONTENT_ENCODING: u32 = 0x6240;
const CONTENT_ENCODING_ORDER: u32 = 0x5031;
const CONTENT_ENCODING_SCOPE: u32 = 0x5032;
const CONTENT_ENCODING_TYPE: u32 = 0x5033;
const CONTENT_COMPRESSION: u32 = 0x5034;
const CONTENT_COMP_ALGO: u32 = 0x4254;
const CONTENT_COMP_SETTINGS: u32 = 0x4255;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const CUES: u32 = 0x1C53_BB6B;
const TAGS: u32 = 0x1254_C367;
const ATTACHMENTS: u32 = 0x1941_A469;
const CHAPTERS: u32 = 0x1043_A770;

/// TrackType of a subtitle track.
const SUBTITLE_TRACK: u64 = 17;

/// The Segment children that end a Cluster of unknown size (rule 4).
const ENDS_A_CLUSTER: [u32; 8] = [CLUSTER, CUES, TAGS, ATTACHMENTS, CHAPTERS, SEEK_HEAD, INFO, TRACKS];

fn not_matroska() -> String {
    NOT_MATROSKA.to_string()
}

/// A variable-length integer at `pos`, not past `end`: its length (one plus the leading zero bits of its first byte,
/// 1 to 8) and its value with the marker bit kept.
fn vint(bytes: &[u8], pos: usize, end: usize) -> Result<(usize, u64), String> {
    let first = *bytes.get(pos).filter(|_| pos < end).ok_or_else(not_matroska)?;
    if first == 0 {
        return Err(not_matroska());
    }
    let length = first.leading_zeros() as usize + 1;
    let stop = pos.checked_add(length).filter(|stop| *stop <= end).ok_or_else(not_matroska)?;
    let value = bytes[pos..stop].iter().fold(0u64, |value, byte| value << 8 | u64::from(*byte));
    Ok((length, value))
}

/// An element header at `pos` inside a parent that ends at `end`: the ID (marker kept, 1 to 4 bytes), where its data
/// starts, and its size (None when unknown, all data bits ones).
fn header(bytes: &[u8], pos: usize, end: usize) -> Result<(u32, usize, Option<u64>), String> {
    let (id_length, id) = vint(bytes, pos, end)?;
    if id_length > 4 {
        return Err(not_matroska());
    }
    let (size_length, marked) = vint(bytes, pos + id_length, end)?;
    let data_bits = 7 * size_length as u32;
    let size = marked & ((1u64 << data_bits) - 1);
    let known = (size != (1u64 << data_bits) - 1).then_some(size);
    Ok((id as u32, pos + id_length + size_length, known))
}

/// The end of an element of known `size` whose data starts at `start`, which must not run past its parent's `end`.
fn data_end(start: usize, size: u64, end: usize) -> Result<usize, String> {
    let stop = usize::try_from(size).ok().and_then(|size| start.checked_add(size));
    stop.filter(|stop| *stop <= end).ok_or_else(not_matroska)
}

/// The children of `bytes[start..end]`, each of known size: (ID, data). An unknown size is refused here.
fn children(bytes: &[u8], start: usize, end: usize) -> Result<Vec<(u32, &[u8])>, String> {
    let mut found = Vec::new();
    let mut pos = start;
    while pos < end {
        let (id, data, size) = header(bytes, pos, end)?;
        let stop = data_end(data, size.ok_or_else(not_matroska)?, end)?;
        found.push((id, &bytes[data..stop]));
        pos = stop;
    }
    Ok(found)
}

/// An unsigned integer element's value: big-endian, 0 to 8 bytes.
fn uint(data: &[u8]) -> Result<u64, String> {
    if data.len() > 8 {
        return Err(not_matroska());
    }
    Ok(data.iter().fold(0u64, |value, byte| value << 8 | u64::from(*byte)))
}

/// A string element's value, without the zero bytes EBML allows after it.
fn string(data: &[u8]) -> String {
    let text = data.iter().position(|byte| *byte == 0).map_or(data, |zero| &data[..zero]);
    String::from_utf8_lossy(text).into_owned()
}

/// What one ContentEncoding does to the data of its scope.
#[derive(Clone, Debug)]
enum Coding {
    Zlib,
    Strip(Vec<u8>),
    /// A content encoding the reader does not undo: the `{kind}` of its refusal.
    Refused(String),
}

#[derive(Clone, Debug)]
struct Encoding {
    order: u64,
    scope: u64,
    coding: Coding,
}

#[derive(Clone, Debug)]
struct Entry<'a> {
    number: u64,
    kind: u64,
    codec_id: String,
    private: &'a [u8],
    /// Highest ContentEncodingOrder first, the order they are undone in.
    encodings: Vec<Encoding>,
}

fn content_encoding(data: &[u8]) -> Result<Encoding, String> {
    let (mut order, mut scope, mut kind, mut algo, mut settings) = (0, 1, 0, 0, Vec::new());
    for (id, child) in children(data, 0, data.len())? {
        match id {
            CONTENT_ENCODING_ORDER => order = uint(child)?,
            CONTENT_ENCODING_SCOPE => scope = uint(child)?,
            CONTENT_ENCODING_TYPE => kind = uint(child)?,
            CONTENT_COMPRESSION => {
                for (id, child) in children(child, 0, child.len())? {
                    match id {
                        CONTENT_COMP_ALGO => algo = uint(child)?,
                        CONTENT_COMP_SETTINGS => settings = child.to_vec(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let coding = match (kind, algo) {
        (0, 0) => Coding::Zlib,
        (0, 3) => Coding::Strip(settings),
        (0, algo) => Coding::Refused(algo.to_string()),
        _ => Coding::Refused("encryption".to_string()),
    };
    Ok(Encoding { order, scope, coding })
}

fn track_entry(data: &[u8]) -> Result<Entry<'_>, String> {
    let (mut number, mut kind, mut codec_id, mut private) = (None, 0, String::new(), &data[..0]);
    let mut encodings = Vec::new();
    for (id, child) in children(data, 0, data.len())? {
        match id {
            TRACK_NUMBER => number = Some(uint(child)?),
            TRACK_TYPE => kind = uint(child)?,
            CODEC_ID => codec_id = string(child),
            CODEC_PRIVATE => private = child,
            CONTENT_ENCODINGS => {
                for (id, child) in children(child, 0, child.len())? {
                    if id == CONTENT_ENCODING {
                        encodings.push(content_encoding(child)?);
                    }
                }
            }
            _ => {}
        }
    }
    encodings.sort_by_key(|encoding| std::cmp::Reverse(encoding.order));
    Ok(Entry { number: number.ok_or_else(not_matroska)?, kind, codec_id, private, encodings })
}

/// Undoes the encodings whose scope has `bit` on `data`, highest order first.
fn decode(data: &[u8], encodings: &[Encoding], bit: u64, track: u64) -> Result<Vec<u8>, String> {
    let mut data = data.to_vec();
    for encoding in encodings.iter().filter(|encoding| encoding.scope & bit != 0) {
        data = match &encoding.coding {
            Coding::Zlib => miniz_oxide::inflate::decompress_to_vec_zlib(&data)
                .map_err(|_| format!("zlib content of track {track} does not inflate"))?,
            Coding::Strip(stripped) => [&stripped[..], &data[..]].concat(),
            Coding::Refused(kind) => {
                return Err(format!(
                    "content encoding {kind} of track {track} is not zlib compression or header stripping"
                ))
            }
        };
    }
    Ok(data)
}

/// The end of a Cluster of unknown size whose data starts at `start`: before the next Segment child that ends a
/// Cluster, or at the Segment's end (rule 4).
fn unknown_cluster_end(bytes: &[u8], start: usize, segment_end: usize) -> Result<usize, String> {
    let mut pos = start;
    while pos < segment_end {
        let (id, data, size) = header(bytes, pos, segment_end)?;
        if ENDS_A_CLUSTER.contains(&id) {
            break;
        }
        pos = data_end(data, size.ok_or_else(not_matroska)?, segment_end)?;
    }
    Ok(pos)
}

/// The Segment's TimestampScale, TrackEntries in order, and Clusters as (data start, data end).
struct Segment<'a> {
    timestamp_scale: u64,
    entries: Vec<Entry<'a>>,
    clusters: Vec<(usize, usize)>,
}

fn segment(bytes: &[u8]) -> Result<Segment<'_>, String> {
    let (id, data, size) = header(bytes, 0, bytes.len())?;
    if id != EBML {
        return Err(not_matroska());
    }
    let header_end = data_end(data, size.ok_or_else(not_matroska)?, bytes.len())?;
    let doc_type = children(bytes, data, header_end)?.into_iter().find(|(id, _)| *id == DOC_TYPE);
    let doc_type = doc_type.map(|(_, data)| string(data));
    if !matches!(doc_type.as_deref(), Some("matroska" | "webm")) {
        return Err(not_matroska());
    }
    let (id, data, size) = header(bytes, header_end, bytes.len())?;
    if id != SEGMENT {
        return Err(not_matroska());
    }
    let end = match size {
        Some(size) => data_end(data, size, bytes.len())?,
        None => bytes.len(),
    };
    let mut found = Segment { timestamp_scale: 1_000_000, entries: Vec::new(), clusters: Vec::new() };
    let mut pos = data;
    while pos < end {
        let (id, data, size) = header(bytes, pos, end)?;
        let stop = match (id, size) {
            (CLUSTER, None) => unknown_cluster_end(bytes, data, end)?,
            (_, None) => return Err(not_matroska()),
            (_, Some(size)) => data_end(data, size, end)?,
        };
        match id {
            INFO => {
                for (id, child) in children(bytes, data, stop)? {
                    if id == TIMESTAMP_SCALE {
                        found.timestamp_scale = uint(child)?;
                    }
                }
            }
            TRACKS => {
                for (id, child) in children(bytes, data, stop)? {
                    if id == TRACK_ENTRY {
                        found.entries.push(track_entry(child)?);
                    }
                }
            }
            CLUSTER => found.clusters.push((data, stop)),
            _ => {}
        }
        pos = stop;
    }
    Ok(found)
}

/// A block's (track number, relative time, flags, frame data); a block cut short is not Matroska.
fn block_parts(body: &[u8]) -> Result<(u64, i16, u8, &[u8]), String> {
    let (length, marked) = vint(body, 0, body.len())?;
    let track = marked & ((1u64 << (7 * length as u32)) - 1);
    let rest = body.get(length..).filter(|rest| rest.len() >= 3).ok_or_else(not_matroska)?;
    Ok((track, i16::from_be_bytes([rest[0], rest[1]]), rest[2], &rest[3..]))
}

/// Reads subtitle stream `n` of a subtitle-only Matroska file, counted from 0 over the tracks of type subtitle in
/// track order, ffprobe's subtitle-relative index (T5-R3). Undoes zlib compression and header stripping with
/// `miniz_oxide`, refuses any other content encoding and laced blocks (design part 10.3, K1). Pure. Def: T5-3. Use:
/// T5-8, T5-11, T5-13.
pub fn read_stream(bytes: &[u8], n: usize) -> Result<SubStream, String> {
    let segment = segment(bytes)?;
    let entry = segment
        .entries
        .iter()
        .filter(|entry| entry.kind == SUBTITLE_TRACK)
        .nth(n)
        .ok_or_else(|| format!("evidence.mks has no subtitle stream {n}"))?;
    let track = entry.number;
    if let Some(kind) = entry.encodings.iter().find_map(|encoding| match &encoding.coding {
        Coding::Refused(kind) => Some(kind),
        _ => None,
    }) {
        return Err(format!("content encoding {kind} of track {track} is not zlib compression or header stripping"));
    }
    let (codec, idx) = match entry.codec_id.as_str() {
        CODEC_PGS => (Some(SubCodec::Pgs), None),
        CODEC_VOBSUB => {
            let private = decode(entry.private, &entry.encodings, 2, track)?;
            (Some(SubCodec::VobSub), Some(parse_idx(&private)?))
        }
        _ => (None, None),
    };
    let scale = segment.timestamp_scale as f64;
    let mut blocks = Vec::new();
    for (start, end) in segment.clusters {
        let mut cluster = None;
        for (id, child) in children(bytes, start, end)? {
            let body = match id {
                CLUSTER_TIMESTAMP => {
                    cluster = Some(uint(child)?);
                    continue;
                }
                SIMPLE_BLOCK => child,
                BLOCK_GROUP => match children(child, 0, child.len())?.into_iter().find(|(id, _)| *id == BLOCK) {
                    Some((_, body)) => body,
                    None => continue,
                },
                _ => continue,
            };
            let cluster = cluster.ok_or_else(not_matroska)?;
            let (number, relative, flags, frame) = block_parts(body)?;
            if number != track {
                continue;
            }
            let pts = (i128::from(cluster) + i128::from(relative)) as f64 * scale / 1_000_000_000.0;
            if flags & 0x06 != 0 {
                return Err(format!("track {track} has a laced block at {pts:.3} s"));
            }
            let payload = decode(frame, &entry.encodings, 1, track)?;
            blocks.push(SubBlock { pts, size: payload.len() as u64, payload });
        }
    }
    Ok(SubStream { track, codec_id: entry.codec_id.clone(), codec, idx, blocks })
}

/// Decimal digits only, as a u32.
fn decimal(text: &str) -> Option<u32> {
    (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())).then(|| text.parse().ok()).flatten()
}

/// Six hex digits, as [R, G, B].
fn colour(text: &str) -> Option<[u8; 3]> {
    if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&text[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// Parses a VobSub idx header's `size:` and `palette:` lines (design part 10.3). Pure. Def: T5-3. Use: T5-3, T5-4.
pub fn parse_idx(codec_private: &[u8]) -> Result<IdxHeader, String> {
    let text = String::from_utf8_lossy(codec_private);
    let lines: Vec<&str> = text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect();
    let field = |name: &str| lines.iter().find_map(|line| line.strip_prefix(name)).map(str::trim);
    let (width, height) = field("size:")
        .and_then(|size| size.split_once('x'))
        .and_then(|(width, height)| Some((decimal(width.trim())?, decimal(height.trim())?)))
        .ok_or("the VobSub idx header has no size line")?;
    let palette: [[u8; 3]; 16] = field("palette:")
        .and_then(|palette| palette.split(',').map(|text| colour(text.trim())).collect::<Option<Vec<_>>>())
        .and_then(|colours| colours.try_into().ok())
        .ok_or("the VobSub idx header has no palette of 16 colours")?;
    Ok(IdxHeader { width, height, palette })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::bitmap::{SubCodec, CODEC_PGS, CODEC_VOBSUB};
    use crate::testsupport::mks::{idx_text, mks, MksBlock, MksEncoding, MksShape, MksTrack, ATTACHMENT_TRAP};

    fn track(number: u64, kind: u64, codec_id: &str, encoding: MksEncoding) -> MksTrack {
        MksTrack {
            number,
            kind,
            codec_id: codec_id.to_string(),
            codec_private: Vec::new(),
            language: "eng".to_string(),
            encoding,
        }
    }

    /// A PGS subtitle track (track type 17) without a content encoding.
    fn pgs(number: u64) -> MksTrack {
        track(number, 17, CODEC_PGS, MksEncoding::None)
    }

    fn block(track: u64, time_ms: i64, payload: &[u8]) -> MksBlock {
        MksBlock::new(track, time_ms, payload.to_vec())
    }

    fn plain(tracks: &[MksTrack], blocks: &[MksBlock]) -> Vec<u8> {
        mks(tracks, blocks, &MksShape::default())
    }

    /// (pts, size, payload) of each block of subtitle stream `n`, in file order.
    fn read(bytes: &[u8], n: usize) -> Vec<(f64, u64, Vec<u8>)> {
        read_stream(bytes, n).unwrap().blocks.into_iter().map(|b| (b.pts, b.size, b.payload)).collect()
    }

    /// 2,029 made-up bytes (the size of phase 0's smallest PGS event, spec 8.7), a pattern zlib compresses well.
    fn event_bytes() -> Vec<u8> {
        (0..2029u32).map(|i| (i % 7) as u8).collect()
    }

    /// Sixteen made-up colours.
    fn palette() -> [[u8; 3]; 16] {
        std::array::from_fn(|i| [i as u8 * 16, 255 - i as u8 * 16, 0x10])
    }

    #[test]
    fn a_simple_block_track_reads_its_pts_and_payload() {
        let bytes = plain(&[pgs(1)], &[block(1, 1_000, b"display-set-a"), block(1, 2_500, b"display-set-bb")]);
        let stream = read_stream(&bytes, 0).unwrap();
        assert_eq!((stream.track, stream.codec_id.as_str(), stream.codec, stream.idx), (1, "S_HDMV/PGS", Some(SubCodec::Pgs), None));
        assert_eq!(read(&bytes, 0), [(1.0, 13, b"display-set-a".to_vec()), (2.5, 14, b"display-set-bb".to_vec())]);
    }

    #[test]
    fn the_pts_is_cluster_plus_block_time_scaled_by_the_timestamp_scale() {
        let blocks = [block(1, 61_103, b"event")];
        assert_eq!((blocks[0].cluster_ms, blocks[0].time_ms - blocks[0].cluster_ms), (61_000, 103));
        assert_eq!(read(&plain(&[pgs(1)], &blocks), 0)[0].0, 61.103);
        let tenth = MksShape { timestamp_scale: Some(100_000), ..MksShape::default() };
        assert_eq!(read(&mks(&[pgs(1)], &blocks, &tenth), 0)[0].0, 6.1103);
    }

    #[test]
    fn zlib_content_is_inflated_and_its_size_is_the_inflated_size() {
        let event = event_bytes();
        let bytes = plain(&[track(1, 17, CODEC_PGS, MksEncoding::Zlib)], &[block(1, 1_000, &event)]);
        assert!(bytes.len() < event.len(), "the writer stored {} bytes, so the payload was not compressed", bytes.len());
        assert_eq!(read(&bytes, 0), [(1.0, 2029, event)]);
    }

    #[test]
    fn header_stripping_puts_the_stripped_bytes_back_first() {
        let stripping = MksEncoding::HeaderStripping(vec![0x50, 0x47, 0x00]);
        let blocks = [block(1, 1_000, b"PG\x00event-one"), block(1, 2_000, b"PG\x00event-two")];
        let bytes = plain(&[track(1, 17, CODEC_PGS, stripping)], &blocks);
        assert!(!bytes.windows(12).any(|w| w == b"PG\x00event-one"), "the writer kept the stripped bytes in the block");
        assert_eq!(read(&bytes, 0), [(1.0, 12, b"PG\x00event-one".to_vec()), (2.0, 12, b"PG\x00event-two".to_vec())]);
    }

    #[test]
    fn another_content_encoding_is_refused() {
        let bytes = plain(&[track(2, 17, CODEC_PGS, MksEncoding::Other(2))], &[block(2, 1_000, b"lzo")]);
        assert_eq!(read_stream(&bytes, 0).unwrap_err(), "content encoding 2 of track 2 is not zlib compression or header stripping");
    }

    #[test]
    fn zlib_content_that_does_not_inflate_is_refused() {
        // Other(0) writes ContentCompAlgo 0 (zlib) and stores the payload as given, so this one is not zlib data.
        let bytes = plain(&[track(1, 17, CODEC_PGS, MksEncoding::Other(0))], &[block(1, 1_000, b"not zlib data")]);
        assert_eq!(read_stream(&bytes, 0).unwrap_err(), "zlib content of track 1 does not inflate");
    }

    #[test]
    fn a_laced_block_is_refused() {
        let tracks = [track(1, 17, "S_TEXT/UTF8", MksEncoding::None), pgs(2)];
        let laced = |track| MksBlock { laced: true, ..block(track, 2_000, b"laced") };
        let own = plain(&tracks, &[block(2, 1_000, b"event"), laced(2)]);
        assert_eq!(read_stream(&own, 1).unwrap_err(), "track 2 has a laced block at 2.000 s");
        let other = plain(&tracks, &[block(2, 1_000, b"event"), laced(1)]);
        assert_eq!(read(&other, 1), [(1.0, 5, b"event".to_vec())]);
    }

    #[test]
    fn stream_n_is_the_nth_subtitle_track_in_track_order() {
        let tracks = [track(1, 17, "S_TEXT/UTF8", MksEncoding::None), track(2, 2, "A_AC3", MksEncoding::None), pgs(3)];
        let bytes = plain(&tracks, &[block(1, 1_000, b"text"), block(2, 1_000, b"audio"), block(3, 1_500, b"bitmap")]);
        let text = read_stream(&bytes, 0).unwrap();
        assert_eq!((text.track, text.codec_id.as_str(), text.codec), (1, "S_TEXT/UTF8", None));
        let bitmap = read_stream(&bytes, 1).unwrap();
        assert_eq!((bitmap.track, bitmap.codec), (3, Some(SubCodec::Pgs)));
        assert_eq!(read(&bytes, 1), [(1.5, 6, b"bitmap".to_vec())]);
        assert_eq!(read_stream(&bytes, 2).unwrap_err(), "evidence.mks has no subtitle stream 2");
    }

    #[test]
    fn the_vobsub_idx_gives_the_frame_size_and_sixteen_colours() {
        let vobsub = MksTrack { codec_private: idx_text(720, 480, &palette()), ..track(1, 17, CODEC_VOBSUB, MksEncoding::Zlib) };
        let stream = read_stream(&plain(&[vobsub], &[block(1, 1_000, b"unit")]), 0).unwrap();
        assert_eq!(stream.codec, Some(SubCodec::VobSub));
        assert_eq!(stream.idx, Some(IdxHeader { width: 720, height: 480, palette: palette() }));
        let mkvmerge_like = [
            "# VobSub index file, v7 (do not modify this line!)",
            "size: 1920x1080",
            "org: 0, 0",
            "scale: 100%, 100%",
            "alpha: 100%",
            "smooth: OFF",
            "fadein/out: 50, 50",
            "align: OFF at LEFT TOP",
            "time offset: 0",
            "forced subs: OFF",
            "palette: 000000, FFFFFF, 101010, 0000ff, 00FF00, ff0000, 808080, 7F7F7F, 010203, 040506, 070809, 0A0B0C, 0d0e0f, 102030, 405060, 708090",
            "custom colors: OFF, tridx: 0000, colors: 000000, 000000, 000000, 000000",
            "",
        ]
        .join("\r\n");
        let idx = parse_idx(mkvmerge_like.as_bytes()).unwrap();
        assert_eq!((idx.width, idx.height), (1920, 1080));
        assert_eq!(idx.palette[..4], [[0, 0, 0], [255, 255, 255], [16, 16, 16], [0, 0, 255]]);
        assert_eq!(idx.palette[15], [0x70, 0x80, 0x90]);
    }

    #[test]
    fn an_idx_header_without_a_size_or_sixteen_colours_is_refused() {
        let sixteen = ["000000"; 16].join(", ");
        let fifteen = ["000000"; 15].join(", ");
        assert_eq!(parse_idx(format!("palette: {sixteen}\n").as_bytes()).unwrap_err(), "the VobSub idx header has no size line");
        assert_eq!(parse_idx(format!("size: 720x480\npalette: {fifteen}\n").as_bytes()).unwrap_err(), "the VobSub idx header has no palette of 16 colours");
        assert_eq!(parse_idx(b"size: 720x480\n").unwrap_err(), "the VobSub idx header has no palette of 16 colours");
        let vobsub = MksTrack { codec_private: b"size: 720x480\n".to_vec(), ..track(1, 17, CODEC_VOBSUB, MksEncoding::None) };
        assert_eq!(read_stream(&plain(&[vobsub], &[block(1, 1_000, b"unit")]), 0).unwrap_err(), "the VobSub idx header has no palette of 16 colours");
    }

    #[test]
    fn a_codec_other_than_pgs_or_vobsub_reads_with_no_codec() {
        let textst = track(1, 17, "S_HDMV/TEXTST", MksEncoding::None);
        let stream = read_stream(&plain(&[textst], &[block(1, 1_000, b"dialog")]), 0).unwrap();
        assert_eq!((stream.codec_id.as_str(), stream.codec, stream.idx), ("S_HDMV/TEXTST", None, None));
        assert_eq!(stream.blocks.len(), 1);
    }

    #[test]
    fn bytes_that_are_not_matroska_are_refused() {
        let good = plain(&[pgs(1)], &[block(1, 1_000, b"event")]);
        let bad: [&[u8]; 3] = [b"", b"RIFF\x24\x00\x00\x00WAVEfmt ", &good[..good.len() - 3]];
        for bytes in bad {
            assert_eq!(read_stream(bytes, 0).unwrap_err(), "not a Matroska file");
        }
    }

    #[test]
    fn a_hand_assembled_file_reads_without_the_test_writer() {
        // EBML header (DocType "matroska"), a Segment of 55 bytes: Info (TimestampScale 1,000,000), Tracks (one
        // TrackEntry: number 1, type 17, CodecID "S_HDMV/PGS"), a Cluster (Timestamp 61,000) with one SimpleBlock
        // (track 1, relative time 103, keyframe flag, payload "abc").
        let parts: [&[u8]; 8] = [
            &[0x1A, 0x45, 0xDF, 0xA3, 0x8B, 0x42, 0x82, 0x88],
            b"matroska",
            &[0x18, 0x53, 0x80, 0x67, 0xB7],
            &[0x15, 0x49, 0xA9, 0x66, 0x87, 0x2A, 0xD7, 0xB1, 0x83, 0x0F, 0x42, 0x40],
            &[0x16, 0x54, 0xAE, 0x6B, 0x94, 0xAE, 0x92, 0xD7, 0x81, 0x01, 0x83, 0x81, 0x11, 0x86, 0x8A],
            b"S_HDMV/PGS",
            &[0x1F, 0x43, 0xB6, 0x75, 0x8D, 0xE7, 0x82, 0xEE, 0x48, 0xA3, 0x87, 0x81, 0x00, 0x67, 0x80],
            b"abc",
        ];
        let bytes = parts.concat();
        let stream = read_stream(&bytes, 0).unwrap();
        assert_eq!((stream.track, stream.codec), (1, Some(SubCodec::Pgs)));
        assert_eq!(read(&bytes, 0), [(61.103, 3, b"abc".to_vec())]);
    }

    #[test]
    fn the_reader_skips_void_and_crc_elements_and_reads_unknown_size_clusters() {
        let shape = MksShape { void_elements: true, crc32: true, unknown_size_clusters: true, ..MksShape::default() };
        let blocks = [block(1, 1_000, b"one"), block(1, 1_400, b"two"), block(1, 5_000, b"three")];
        assert_eq!(
            read(&mks(&[pgs(1)], &blocks, &shape), 0),
            [(1.0, 3, b"one".to_vec()), (1.4, 3, b"two".to_vec()), (5.0, 5, b"three".to_vec())]
        );
    }

    #[test]
    fn cues_tags_and_attachments_around_the_clusters_are_skipped() {
        let blocks = [block(1, 1_000, b"one"), block(1, 5_000, b"two")];
        for unknown_size_clusters in [false, true] {
            let shape = MksShape { cues_and_tags: true, unknown_size_clusters, ..MksShape::default() };
            let bytes = mks(&[pgs(1)], &blocks, &shape);
            assert!(bytes.windows(6).any(|w| w == &ATTACHMENT_TRAP), "the writer wrote no attachment");
            assert_eq!(read(&bytes, 0), [(1.0, 3, b"one".to_vec()), (5.0, 3, b"two".to_vec())]);
        }
    }

    #[test]
    fn a_block_group_with_a_duration_reads_as_its_block() {
        let grouped = MksBlock { group: true, ..block(1, 2_000, b"grouped") };
        let bytes = plain(&[pgs(1)], &[block(1, 1_000, b"simple"), grouped]);
        assert_eq!(read(&bytes, 0), [(1.0, 6, b"simple".to_vec()), (2.0, 7, b"grouped".to_vec())]);
    }

    #[test]
    fn a_negative_relative_timestamp_is_the_cluster_time_minus_it() {
        let early = MksBlock { cluster_ms: 61_000, ..block(1, 60_750, b"early") };
        let bytes = plain(&[pgs(1)], &[early, block(1, 61_200, b"later")]);
        assert_eq!(read(&bytes, 0), [(60.75, 5, b"early".to_vec()), (61.2, 5, b"later".to_vec())]);
    }
}
