//! A made-up Matroska writer (design part 10.3): the subtitle-only file mkvmerge writes, in the shapes its reader
//! must handle (Void and CRC-32 elements, unknown-size clusters, Cues, Tags and Attachments around the clusters,
//! block groups, zlib and header-stripping content encodings), so that no test byte comes from a real disc (RP21).

/// The content encoding a made-up track declares, which the writer applies to its block payloads (scope 1): none,
/// zlib (ContentCompAlgo 0), header stripping (3, of the given bytes), or another ContentCompAlgo number stored with
/// the payload as given (design part 10.3, K1). Def: T5-3. Use: T5-3, T5-4, T5-8, T5-9, T5-13, T5-24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MksEncoding {
    None,
    Zlib,
    HeaderStripping(Vec<u8>),
    Other(u64),
}

/// One made-up TrackEntry: TrackNumber (also its TrackUID), TrackType (`kind`, 17 for subtitles), CodecID,
/// CodecPrivate (written only when not empty), Language and content encoding (design part 10.3). Def: T5-3. Use:
/// T5-3, T5-4, T5-8, T5-9, T5-13, T5-24.
#[derive(Clone, Debug)]
pub struct MksTrack {
    pub number: u64,
    pub kind: u64,
    pub codec_id: String,
    pub codec_private: Vec<u8>,
    pub language: String,
    pub encoding: MksEncoding,
}

/// One made-up block: its track, its time and its Cluster's time in milliseconds (at the default timestamp scale),
/// its payload before the content encoding, and whether it is written laced or as a BlockGroup with a BlockDuration
/// (design part 10.3). Def: T5-3. Use: T5-3, T5-4, T5-8, T5-9, T5-13, T5-24.
#[derive(Clone, Debug)]
pub struct MksBlock {
    pub track: u64,
    pub time_ms: i64,
    pub cluster_ms: i64,
    pub payload: Vec<u8>,
    pub laced: bool,
    pub group: bool,
}

impl MksBlock {
    /// A SimpleBlock, not laced, in the Cluster of its whole second (design part 10.3). Def: T5-3. Use: T5-3, T5-4,
    /// T5-8, T5-9, T5-13, T5-24.
    pub fn new(track: u64, time_ms: i64, payload: Vec<u8>) -> MksBlock {
        MksBlock { track, time_ms, cluster_ms: time_ms.div_euclid(1000) * 1000, payload, laced: false, group: false }
    }
}

/// The optional shapes of a made-up file: Void elements, CRC-32 elements, Clusters of unknown size, the SeekHead,
/// Attachments, Cues and Tags around the Clusters, and a TimestampScale other than 1,000,000 (design part 10.3).
/// Def: T5-3. Use: T5-3, T5-4, T5-8, T5-9, T5-13, T5-24.
#[derive(Clone, Copy, Debug, Default)]
pub struct MksShape {
    pub void_elements: bool,
    pub crc32: bool,
    pub unknown_size_clusters: bool,
    pub cues_and_tags: bool,
    pub timestamp_scale: Option<u64>,
}

/// The FileData of the attachment `cues_and_tags` writes: the bytes of a SimpleBlock of track 1, which a reader that
/// scanned the Attachments instead of skipping them by size would find (design part 10.3). Def: T5-3. Use: T5-3,
/// T5-4, T5-8, T5-9, T5-13, T5-24.
pub const ATTACHMENT_TRAP: [u8; 6] = [0xA3, 0x84, 0x81, 0x00, 0x00, 0x80];

/// The eight bytes of an unknown element size.
const UNKNOWN_SIZE: [u8; 8] = [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

/// A size as the shortest EBML variable-length integer that holds it (all data bits ones is the unknown size, so it
/// takes the next length).
fn vint(size: u64) -> Vec<u8> {
    let length = (1..=8u32).find(|l| size < (1u64 << (7 * l)) - 1).expect("an element size below 2^56 - 1");
    let marked = size | (1u64 << (7 * length));
    marked.to_be_bytes()[8 - length as usize..].to_vec()
}

/// An unsigned integer, big-endian in the fewest bytes, at least one.
fn uint(value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let first = bytes.iter().position(|b| *b != 0).unwrap_or(7);
    bytes[first..].to_vec()
}

/// An element of known size.
fn element(id: &[u8], body: &[u8]) -> Vec<u8> {
    [id, &vint(body.len() as u64)[..], body].concat()
}

fn uint_element(id: &[u8], value: u64) -> Vec<u8> {
    element(id, &uint(value))
}

/// A Void element of 4 zero bytes.
fn void() -> Vec<u8> {
    element(&[0xEC], &[0; 4])
}

/// A CRC-32 element of 4 zero bytes.
fn crc32() -> Vec<u8> {
    element(&[0xBF], &[0; 4])
}

fn track_entry(track: &MksTrack) -> Vec<u8> {
    let mut body = [
        uint_element(&[0xD7], track.number),
        uint_element(&[0x73, 0xC5], track.number),
        uint_element(&[0x83], track.kind),
        element(&[0x86], track.codec_id.as_bytes()),
        element(&[0x22, 0xB5, 0x9C], track.language.as_bytes()),
    ]
    .concat();
    if !track.codec_private.is_empty() {
        body.extend(element(&[0x63, 0xA2], &track.codec_private));
    }
    let compression = match &track.encoding {
        MksEncoding::None => None,
        MksEncoding::Zlib => Some(uint_element(&[0x42, 0x54], 0)),
        MksEncoding::HeaderStripping(stripped) => {
            Some([uint_element(&[0x42, 0x54], 3), element(&[0x42, 0x55], stripped)].concat())
        }
        MksEncoding::Other(algo) => Some(uint_element(&[0x42, 0x54], *algo)),
    };
    if let Some(compression) = compression {
        let encoding = [
            uint_element(&[0x50, 0x31], 0),
            uint_element(&[0x50, 0x32], 1),
            uint_element(&[0x50, 0x33], 0),
            element(&[0x50, 0x34], &compression),
        ]
        .concat();
        body.extend(element(&[0x6D, 0x80], &element(&[0x62, 0x40], &encoding)));
    }
    element(&[0xAE], &body)
}

/// A block's stored payload: its track's content encoding applied.
fn stored(payload: &[u8], encoding: &MksEncoding) -> Vec<u8> {
    match encoding {
        MksEncoding::Zlib => miniz_oxide::deflate::compress_to_vec_zlib(payload, 6),
        MksEncoding::HeaderStripping(stripped) => {
            assert!(payload.starts_with(stripped), "a header-stripped payload must start with the stripped bytes");
            payload[stripped.len()..].to_vec()
        }
        MksEncoding::None | MksEncoding::Other(_) => payload.to_vec(),
    }
}

fn block(block: &MksBlock, tracks: &[MksTrack]) -> Vec<u8> {
    assert!(block.track < 127, "the writer writes a track number below 127 as a 1-byte VINT");
    let relative = i16::try_from(block.time_ms - block.cluster_ms).expect("a block time within an i16 of its cluster");
    let flags = (if block.group { 0x00 } else { 0x80 }) | (if block.laced { 0x02 } else { 0x00 });
    let encoding = tracks.iter().find(|t| t.number == block.track).map_or(&MksEncoding::None, |t| &t.encoding);
    let payload = stored(&block.payload, encoding);
    let body = [&[0x80 | block.track as u8][..], &relative.to_be_bytes()[..], &[flags][..], &payload[..]].concat();
    if block.group {
        element(&[0xA0], &[element(&[0xA1], &body), uint_element(&[0x9B], 2_000)].concat())
    } else {
        element(&[0xA3], &body)
    }
}

/// A made-up subtitle-only Matroska file in the shapes mkvmerge writes: the EBML header, one Segment of known size
/// holding Info, Tracks (one TrackEntry per track, in order) and the Clusters (consecutive blocks of equal
/// `cluster_ms` share one), with the optional shapes of `shape` (design part 10.3). Def: T5-3. Use: T5-3, T5-4, T5-8,
/// T5-9, T5-13, T5-24.
pub fn mks(tracks: &[MksTrack], blocks: &[MksBlock], shape: &MksShape) -> Vec<u8> {
    let header = [
        uint_element(&[0x42, 0x86], 1),
        uint_element(&[0x42, 0xF7], 1),
        uint_element(&[0x42, 0xF2], 4),
        uint_element(&[0x42, 0xF3], 8),
        element(&[0x42, 0x82], b"matroska"),
        uint_element(&[0x42, 0x87], 4),
        uint_element(&[0x42, 0x85], 2),
    ]
    .concat();
    let crc = |on: bool| if on { crc32() } else { Vec::new() };
    let mut segment = Vec::new();
    if shape.cues_and_tags {
        let seek = [element(&[0x53, 0xAB], &[0x15, 0x49, 0xA9, 0x66]), uint_element(&[0x53, 0xAC], 0)].concat();
        segment.extend(element(&[0x11, 0x4D, 0x9B, 0x74], &element(&[0x4D, 0xBB], &seek)));
    }
    if shape.void_elements {
        segment.extend(void());
    }
    let info = [
        crc(shape.crc32),
        uint_element(&[0x2A, 0xD7, 0xB1], shape.timestamp_scale.unwrap_or(1_000_000)),
        element(&[0x4D, 0x80], b"lazymkv testsupport"),
    ]
    .concat();
    segment.extend(element(&[0x15, 0x49, 0xA9, 0x66], &info));
    let entries: Vec<u8> = tracks.iter().flat_map(track_entry).collect();
    segment.extend(element(&[0x16, 0x54, 0xAE, 0x6B], &[crc(shape.crc32), entries].concat()));
    if shape.cues_and_tags {
        let file = [
            element(&[0x46, 0x6E], b"trap.bin"),
            element(&[0x46, 0x60], b"application/octet-stream"),
            element(&[0x46, 0x5C], &ATTACHMENT_TRAP),
            uint_element(&[0x46, 0xAE], 1),
        ]
        .concat();
        segment.extend(element(&[0x19, 0x41, 0xA4, 0x69], &element(&[0x61, 0xA7], &file)));
    }
    for cluster in blocks.chunk_by(|a, b| a.cluster_ms == b.cluster_ms) {
        let cluster_ms = cluster[0].cluster_ms;
        assert!(cluster_ms >= 0, "a cluster time is not negative");
        let mut body = crc(shape.crc32);
        if shape.void_elements {
            body.extend(void());
        }
        body.extend(uint_element(&[0xE7], cluster_ms as u64));
        for one in cluster {
            body.extend(block(one, tracks));
        }
        let id = [0x1F, 0x43, 0xB6, 0x75];
        if shape.unknown_size_clusters {
            segment.extend([&id[..], &UNKNOWN_SIZE[..], &body[..]].concat());
        } else {
            segment.extend(element(&id, &body));
        }
    }
    if shape.cues_and_tags {
        let first_cluster = blocks.first().map_or(0, |b| b.cluster_ms.max(0) as u64);
        let first_track = tracks.first().map_or(0, |t| t.number);
        let positions = [uint_element(&[0xF7], first_track), uint_element(&[0xF1], 0)].concat();
        let point = [uint_element(&[0xB3], first_cluster), element(&[0xB7], &positions)].concat();
        segment.extend(element(&[0x1C, 0x53, 0xBB, 0x6B], &element(&[0xBB], &point)));
        let simple = [element(&[0x45, 0xA3], b"TITLE"), element(&[0x44, 0x87], b"made-up")].concat();
        let tag = [element(&[0x63, 0xC0], &[]), element(&[0x67, 0xC8], &simple)].concat();
        segment.extend(element(&[0x12, 0x54, 0xC3, 0x67], &element(&[0x73, 0x73], &tag)));
    }
    [element(&[0x1A, 0x45, 0xDF, 0xA3], &header), element(&[0x18, 0x53, 0x80, 0x67], &segment)].concat()
}

/// A VobSub idx header of three lines: the index file's first line, `size: {width}x{height}`, and `palette: ` with
/// the sixteen colours as lower-case `rrggbb` joined by `, ` (design part 10.3). Def: T5-3. Use: T5-3, T5-4, T5-8.
pub fn idx_text(width: u32, height: u32, palette: &[[u8; 3]; 16]) -> Vec<u8> {
    let colours: Vec<String> = palette.iter().map(|[r, g, b]| format!("{r:02x}{g:02x}{b:02x}")).collect();
    format!(
        "# VobSub index file, v7 (do not modify this line!)\nsize: {width}x{height}\npalette: {}\n",
        colours.join(", ")
    )
    .into_bytes()
}
