//! Blu-ray playlists (`.mpls`), read for the menu-role increment (menu maps design 5.4a): a playlist's play items and
//! the first item's audio and subtitle stream tables, so a later task can tie a menu choice to a stream. The layout is
//! the spike's `tools/bd.py` `parse_mpls`, which read every captured disc; all integers are big-endian. Every read is
//! bounds-checked and every offset sum is checked, so no input makes anything panic. No I/O: `parse_playlist` takes
//! the file's bytes.

/// One play item: its clip's five-character name, and its in and out time in 45 kHz ticks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayItem {
    /// The clip's five-character name.
    pub clip: String,
    /// The in time, in 45 kHz ticks.
    pub in_time: u32,
    /// The out time, in 45 kHz ticks.
    pub out_time: u32,
}

/// One entry of a stream table: the stream's PID, its coding type, its language as the disc writes it ("" if none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistStream {
    /// The stream's PID.
    pub pid: u16,
    /// The stream's coding type (the attributes' first byte).
    pub coding: u8,
    /// The language as the disc writes it, "" if none.
    pub language: String,
}

/// A parsed playlist: its play items, the first item's primary audio and PG tables (entry n is `[n - 1]`), and whether
/// every play item carries those same two tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Playlist {
    /// The play items, in order.
    pub items: Vec<PlayItem>,
    /// The first item's primary audio table; entry n is `[n - 1]`.
    pub audio: Vec<PlaylistStream>,
    /// The first item's PG table; entry n is `[n - 1]`.
    pub subtitles: Vec<PlaylistStream>,
    /// True when every play item carries the first item's two tables.
    pub uniform: bool,
}

impl Playlist {
    /// The playlist's length in 45 kHz ticks: the sum of `out_time - in_time`, an item with out before in counting 0.
    pub fn ticks(&self) -> u64 {
        self.items.iter().map(|item| u64::from(item.out_time.saturating_sub(item.in_time))).sum()
    }
}

/// The error for a read that starts at `offset` and cannot be satisfied.
fn cut_short(offset: usize) -> String {
    format!("the playlist is cut short at byte {offset}")
}

/// `a + b`, or the cut-short error at `a` if the sum does not fit.
fn plus(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b).ok_or_else(|| cut_short(a))
}

/// The playlist's bytes with bounds-checked reads.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    /// The `n` bytes at `offset`, or the cut-short error at `offset`.
    fn bytes(&self, offset: usize, n: usize) -> Result<&[u8], String> {
        let end = plus(offset, n)?;
        self.0.get(offset..end).ok_or_else(|| cut_short(offset))
    }

    /// The byte at `offset`.
    fn u8(&self, offset: usize) -> Result<u8, String> {
        Ok(self.bytes(offset, 1)?[0])
    }

    /// The big-endian u16 at `offset`.
    fn u16(&self, offset: usize) -> Result<u16, String> {
        let b = self.bytes(offset, 2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// The big-endian u32 at `offset`.
    fn u32(&self, offset: usize) -> Result<u32, String> {
        let b = self.bytes(offset, 4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// The language inside an attribute span (`attrs[0]` is the coding type): 3 bytes after the coding type and one more
/// byte for the audio and text codings, 3 bytes right after it for the graphics codings 0x90 and 0x91, else "". A span
/// too short to hold the language gives "".
fn language(attrs: &[u8]) -> String {
    let skip = match attrs.first() {
        Some(0x03 | 0x04 | 0x80..=0x86 | 0xA1 | 0xA2 | 0x92) => 2,
        Some(0x90 | 0x91) => 1,
        _ => return String::new(),
    };
    match attrs.get(skip..skip + 3) {
        Some(text) => String::from_utf8_lossy(text).into_owned(),
        None => String::new(),
    }
}

/// Reads `count` stream entries from `e`; returns them and the offset after the last. `item` is the play item's index.
fn read_entries(r: &Reader, mut e: usize, count: usize, item: usize) -> Result<(Vec<PlaylistStream>, usize), String> {
    let mut streams = Vec::new();
    for _ in 0..count {
        let l = usize::from(r.u8(e)?);
        let kind = r.u8(plus(e, 1)?)?;
        let pid_at = match kind {
            1 => plus(e, 2)?,
            2 => plus(e, 4)?,
            3 | 4 => plus(e, 3)?,
            other => return Err(format!("play item {item}: a stream entry of unknown type {other}")),
        };
        let pid = r.u16(pid_at)?;
        let a = plus(plus(e, 1)?, l)?;
        let al = usize::from(r.u8(a)?);
        if al == 0 {
            return Err(format!("play item {item}: a stream entry without a coding type"));
        }
        let attrs = r.bytes(plus(a, 1)?, al)?;
        streams.push(PlaylistStream { pid, coding: attrs[0], language: language(attrs) });
        e = plus(plus(a, 1)?, al)?;
    }
    Ok((streams, e))
}

/// What reading one play item gives: the item, its audio table and its subtitle table.
type ItemRead = (PlayItem, Vec<PlaylistStream>, Vec<PlaylistStream>);

/// One play item read from offset `o`: the item, its audio table and its subtitle table; `item` is its index.
fn read_item(r: &Reader, o: usize, item: usize) -> Result<ItemRead, String> {
    let s = plus(o, 2)?;
    let clip = String::from_utf8_lossy(r.bytes(s, 5)?).into_owned();
    let flags = r.u16(plus(s, 9)?)?;
    let in_time = r.u32(plus(s, 12)?)?;
    let out_time = r.u32(plus(s, 16)?)?;
    let mut p = plus(s, 32)?;
    if flags & 0x10 != 0 {
        let angles = usize::from(r.u8(p)?);
        // An angle count of 0 is read as 1: `saturating_sub` gives no extra angle clip, never an underflow.
        p = plus(p, 2 + 10 * angles.saturating_sub(1))?;
    }
    let q = plus(p, 4)?;
    let count = |at: usize| -> Result<usize, String> { Ok(usize::from(r.u8(plus(q, at)?)?)) };
    let (video, audio, pg, pip) = (count(0)?, count(1)?, count(2)?, count(6)?);
    let (_, e) = read_entries(r, plus(q, 12)?, video, item)?;
    let (audio, e) = read_entries(r, e, audio, item)?;
    let (mut subtitles, _) = read_entries(r, e, pg + pip, item)?;
    subtitles.truncate(pg);
    Ok((PlayItem { clip, in_time, out_time }, audio, subtitles))
}

/// Reads a playlist from its file's bytes. Errors are plain sentences; none panics on any input.
pub fn parse_playlist(bytes: &[u8]) -> Result<Playlist, String> {
    let r = Reader(bytes);
    if r.bytes(0, 4)? != b"MPLS" {
        return Err("not a playlist: the file does not start with MPLS".to_string());
    }
    let start = usize::try_from(r.u32(8)?).map_err(|_| cut_short(8))?;
    let count = usize::from(r.u16(plus(start, 6)?)?);
    if count == 0 {
        return Err("the playlist has no play item".to_string());
    }
    let mut o = plus(start, 10)?;
    let mut items = Vec::new();
    let mut first: Option<(Vec<PlaylistStream>, Vec<PlaylistStream>)> = None;
    let mut uniform = true;
    for index in 0..count {
        let len = usize::from(r.u16(o)?);
        let (item, audio, subtitles) = read_item(&r, o, index)?;
        o = plus(plus(o, len)?, 2)?;
        match &first {
            None => first = Some((audio, subtitles)),
            Some((a, s)) => uniform &= *a == audio && *s == subtitles,
        }
        items.push(item);
    }
    let (audio, subtitles) = first.unwrap_or_default();
    Ok(Playlist { items, audio, subtitles, uniform })
}

/// Test support: one play item of a made-up playlist, with its audio tables `(pid, coding, language)` and its
/// subtitle tables `(pid, language)`.
#[cfg(any(test, feature = "testsupport"))]
pub struct MadeUpItem {
    /// The clip's five-character name.
    pub clip: &'static str,
    /// The in time, in 45 kHz ticks.
    pub in_time: u32,
    /// The out time, in 45 kHz ticks.
    pub out_time: u32,
    /// The audio table, `(pid, coding, language)` per entry.
    pub audio: Vec<(u16, u8, &'static str)>,
    /// The subtitle table, `(pid, language)` per entry (coded 0x90).
    pub subtitles: Vec<(u16, &'static str)>,
}

/// Test support: the bytes of a made-up playlist in the layout `parse_playlist` reads (header `MPLS0200`, the play
/// list block at 40, type 1 stream entries with 5-byte attributes, subtitles coded 0x90).
#[cfg(any(test, feature = "testsupport"))]
pub fn made_up_playlist(items: &[MadeUpItem]) -> Vec<u8> {
    /// The first three bytes of `text`, padded with spaces.
    fn lang(text: &str) -> [u8; 3] {
        let mut out = [b' '; 3];
        for (slot, byte) in out.iter_mut().zip(text.bytes()) {
            *slot = byte;
        }
        out
    }
    /// A type 1 stream entry for `pid` followed by its attributes `attrs` (the coding type first).
    fn entry(pid: u16, attrs: [u8; 5]) -> Vec<u8> {
        let mut out = vec![9, 1];
        out.extend(pid.to_be_bytes());
        out.extend([0; 6]);
        out.push(5);
        out.extend(attrs);
        out
    }
    let mut out = b"MPLS0200".to_vec();
    out.extend(40u32.to_be_bytes());
    out.resize(40, 0);
    out.extend(0u32.to_be_bytes());
    out.extend(0u16.to_be_bytes());
    out.extend((items.len() as u16).to_be_bytes());
    out.extend(0u16.to_be_bytes());
    for item in items {
        let mut entries = Vec::new();
        for (pid, coding, text) in &item.audio {
            let l = lang(text);
            entries.extend(entry(*pid, [*coding, 0, l[0], l[1], l[2]]));
        }
        for (pid, text) in &item.subtitles {
            let l = lang(text);
            entries.extend(entry(*pid, [0x90, l[0], l[1], l[2], 0]));
        }
        let mut body = Vec::new();
        body.extend(item.clip.as_bytes());
        body.extend(b"M2TS");
        body.extend(0u16.to_be_bytes());
        body.push(0);
        body.extend(item.in_time.to_be_bytes());
        body.extend(item.out_time.to_be_bytes());
        body.resize(32, 0);
        body.extend(((14 + entries.len()) as u16).to_be_bytes());
        body.extend([0; 2]);
        body.extend([0, item.audio.len() as u8, item.subtitles.len() as u8, 0, 0, 0, 0]);
        body.extend([0; 5]);
        body.extend(entries);
        out.extend((body.len() as u16).to_be_bytes());
        out.extend(body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up item with audio and subtitle tables.
    fn item(clip: &'static str, audio: &[(u16, u8, &'static str)], subtitles: &[(u16, &'static str)]) -> MadeUpItem {
        MadeUpItem { clip, in_time: 0, out_time: 90, audio: audio.to_vec(), subtitles: subtitles.to_vec() }
    }

    /// The offset of the first play item in a made-up playlist (the block at 40, then 10 bytes).
    const FIRST_ITEM: usize = 50;
    /// The offset of the first play item's stream table (the item, 2 bytes of length, then 32).
    const TABLE: usize = FIRST_ITEM + 2 + 32;
    /// The offset of the first stream entry of a made-up item (the table's 4 bytes and 12 of counts).
    const ENTRIES: usize = TABLE + 16;

    #[test]
    fn a_made_up_playlist_reads_back_its_items_and_tables() {
        let bytes = made_up_playlist(&[MadeUpItem {
            clip: "00001",
            in_time: 1000,
            out_time: 46000,
            audio: vec![(0x1100, 0x83, "eng"), (0x1101, 0x81, "fra")],
            subtitles: vec![(0x1200, "eng")],
        }]);
        let playlist = parse_playlist(&bytes).unwrap();
        assert_eq!(playlist.items, vec![PlayItem { clip: "00001".into(), in_time: 1000, out_time: 46000 }]);
        let stream = |pid, coding, language: &str| PlaylistStream { pid, coding, language: language.into() };
        assert_eq!(playlist.audio, vec![stream(0x1100, 0x83, "eng"), stream(0x1101, 0x81, "fra")]);
        assert_eq!(playlist.subtitles, vec![stream(0x1200, 0x90, "eng")]);
        assert!(playlist.uniform);
        assert_eq!(playlist.ticks(), 45000);
    }

    #[test]
    fn two_items_with_the_same_tables_are_uniform_and_differing_ones_are_not() {
        let audio = [(0x1100, 0x83, "eng")];
        let same =
            made_up_playlist(&[item("00001", &audio, &[(0x1200, "eng")]), item("00002", &audio, &[(0x1200, "eng")])]);
        assert!(parse_playlist(&same).unwrap().uniform);
        let other_audio = made_up_playlist(&[
            item("00001", &audio, &[(0x1200, "eng")]),
            item("00002", &[(0x1100, 0x83, "fra")], &[(0x1200, "eng")]),
        ]);
        assert!(!parse_playlist(&other_audio).unwrap().uniform);
        let other_subtitles =
            made_up_playlist(&[item("00001", &audio, &[(0x1200, "eng")]), item("00002", &audio, &[])]);
        assert!(!parse_playlist(&other_subtitles).unwrap().uniform);
    }

    #[test]
    fn a_multi_angle_item_reads_its_table_after_the_angle_clips() {
        let mut bytes = made_up_playlist(&[item("00001", &[(0x1100, 0x83, "eng")], &[(0x1200, "eng")])]);
        let s = FIRST_ITEM + 2;
        bytes[s + 10] |= 0x10;
        let mut block = vec![0u8; 12];
        block[0] = 2;
        bytes.splice(TABLE..TABLE, block);
        let len = u16::from_be_bytes([bytes[FIRST_ITEM], bytes[FIRST_ITEM + 1]]) + 12;
        bytes[FIRST_ITEM..FIRST_ITEM + 2].copy_from_slice(&len.to_be_bytes());
        let playlist = parse_playlist(&bytes).unwrap();
        assert_eq!(playlist.audio.len(), 1);
        assert_eq!(playlist.audio[0].pid, 0x1100);
        assert_eq!(playlist.subtitles[0].language, "eng");
    }

    #[test]
    fn a_secondary_stream_entry_reads_its_pid_at_its_own_offset() {
        let mut bytes = made_up_playlist(&[item("00001", &[(0, 0x83, "eng"), (0, 0x83, "fra")], &[])]);
        bytes[ENTRIES + 1] = 2;
        bytes[ENTRIES + 4..ENTRIES + 6].copy_from_slice(&0x1111u16.to_be_bytes());
        bytes[ENTRIES + 16 + 1] = 3;
        bytes[ENTRIES + 16 + 3..ENTRIES + 16 + 5].copy_from_slice(&0x2222u16.to_be_bytes());
        let playlist = parse_playlist(&bytes).unwrap();
        let pids: Vec<u16> = playlist.audio.iter().map(|stream| stream.pid).collect();
        assert_eq!(pids, vec![0x1111, 0x2222]);
    }

    #[test]
    fn text_that_is_not_a_playlist_is_refused() {
        assert_eq!(parse_playlist(b"MPLX0200").unwrap_err(), "not a playlist: the file does not start with MPLS");
        assert_eq!(parse_playlist(b"").unwrap_err(), "the playlist is cut short at byte 0");
    }

    #[test]
    fn every_truncation_of_a_playlist_is_refused_without_a_panic() {
        let audio = [(0x1100, 0x83, "eng")];
        let two = [item("00001", &audio, &[(0x1200, "eng")]), item("00002", &audio, &[(0x1200, "eng")])];
        let bytes = made_up_playlist(&two);
        assert!(parse_playlist(&bytes).is_ok());
        for n in 0..bytes.len() {
            assert!(parse_playlist(&bytes[..n]).is_err(), "a playlist cut at {n} bytes was read");
        }
    }

    #[test]
    fn a_cut_short_playlist_names_the_start_of_the_read_that_failed() {
        let audio = [(0x1100, 0x83, "eng")];
        let bytes = made_up_playlist(&[item("00001", &audio, &[]), item("00002", &audio, &[])]);
        let second = FIRST_ITEM + 2 + usize::from(u16::from_be_bytes([bytes[FIRST_ITEM], bytes[FIRST_ITEM + 1]]));
        for n in [second, second + 1] {
            let expected = format!("the playlist is cut short at byte {second}");
            assert_eq!(parse_playlist(&bytes[..n]).unwrap_err(), expected, "cut at {n}");
        }
        for n in 1..4 {
            assert_eq!(parse_playlist(&bytes[..n]).unwrap_err(), "the playlist is cut short at byte 0");
        }
    }

    #[test]
    fn a_playlist_without_play_items_is_refused() {
        assert_eq!(parse_playlist(&made_up_playlist(&[])).unwrap_err(), "the playlist has no play item");
    }

    #[test]
    fn an_unknown_stream_entry_type_is_refused_naming_its_item() {
        let audio = [(0x1100, 0x83, "eng")];
        let mut bytes = made_up_playlist(&[item("00001", &audio, &[]), item("00002", &audio, &[])]);
        let second = FIRST_ITEM + 2 + usize::from(u16::from_be_bytes([bytes[FIRST_ITEM], bytes[FIRST_ITEM + 1]]));
        bytes[second + 2 + 32 + 16 + 1] = 9;
        assert_eq!(parse_playlist(&bytes).unwrap_err(), "play item 1: a stream entry of unknown type 9");
    }

    #[test]
    fn an_items_length_is_honoured_when_padding_follows_its_table() {
        let audio = [(0x1100, 0x83, "eng")];
        let mut bytes = made_up_playlist(&[item("00001", &audio, &[]), item("00002", &audio, &[])]);
        let len = u16::from_be_bytes([bytes[FIRST_ITEM], bytes[FIRST_ITEM + 1]]);
        let end = FIRST_ITEM + 2 + usize::from(len);
        bytes.splice(end..end, [0xFF; 4]);
        bytes[FIRST_ITEM..FIRST_ITEM + 2].copy_from_slice(&(len + 4).to_be_bytes());
        let playlist = parse_playlist(&bytes).unwrap();
        let clips: Vec<&str> = playlist.items.iter().map(|item| item.clip.as_str()).collect();
        assert_eq!(clips, vec!["00001", "00002"]);
        assert!(playlist.uniform);
    }

    #[test]
    fn a_stream_entry_without_a_coding_type_is_refused() {
        let mut bytes = made_up_playlist(&[item("00001", &[(0x1100, 0x83, "eng")], &[])]);
        bytes[ENTRIES + 10] = 0;
        assert_eq!(parse_playlist(&bytes).unwrap_err(), "play item 0: a stream entry without a coding type");
    }

    #[test]
    fn an_item_with_its_out_time_before_its_in_time_counts_no_ticks() {
        let mut backwards = item("00001", &[], &[]);
        (backwards.in_time, backwards.out_time) = (500, 100);
        let playlist = parse_playlist(&made_up_playlist(&[backwards])).unwrap();
        assert_eq!(playlist.ticks(), 0);
    }
}
