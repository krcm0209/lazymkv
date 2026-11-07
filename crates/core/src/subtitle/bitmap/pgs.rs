//! The PGS (Presentation Graphic Stream) decoder of design part 10.4: display sets of composition, window, palette,
//! object and end segments, kept as one epoch's palettes and objects.

use std::collections::BTreeMap;

use crate::subtitle::bitmap::{Bitmap, Object};

const COMPOSITION: u8 = 0x16;
const WINDOW: u8 = 0x17;
const PALETTE: u8 = 0x14;
const OBJECT: u8 = 0x15;
const END: u8 = 0x80;

/// The composition state bits of an acquisition point (`0x40`) or an epoch start (`0x80`), which forget the epoch.
const EPOCH_RESET: u8 = 0xC0;
/// The placement flag of a cropped object.
const CROPPED: u8 = 0x80;
/// The sequence flags of an object segment: its first and its last.
const FIRST: u8 = 0x80;
const LAST: u8 = 0x40;

/// The 256 `[Y, alpha]` entries of one palette.
type Palette = Vec<[u8; 2]>;

/// One object of the epoch, decoded: width, height and its palette indices row by row.
#[derive(Debug)]
struct Decoded {
    width: u32,
    height: u32,
    indices: Vec<u8>,
}

/// The object whose run-length data is being joined across its segments.
#[derive(Debug)]
struct Joining {
    id: u16,
    /// The declared data length, less the 4 bytes of width and height.
    expected: u32,
    width: u32,
    height: u32,
    data: Vec<u8>,
}

/// One placement of the composition: object id, x, y and the crop as `[x, y, width, height]`.
#[derive(Debug)]
struct Placement {
    id: u16,
    x: u32,
    y: u32,
    crop: Option<[u32; 4]>,
}

/// The composition segment's fields the event uses.
#[derive(Debug)]
struct Composition {
    video_width: u32,
    video_height: u32,
    palette: u8,
    placements: Vec<Placement>,
}

/// The PGS decoder of design part 10.4: one per stream, fed each event block's payload in `pts` order (only
/// packets larger than `sub_clear_max_bytes`, spec 8.7), keeping the epoch's palettes and objects across blocks.
/// Pure. Def: T5-4. Use: T5-8, T5-11.
#[derive(Debug, Default)]
pub struct PgsDecoder {
    palettes: BTreeMap<u8, Palette>,
    objects: BTreeMap<u16, Decoded>,
    joining: Option<Joining>,
}

/// A big-endian 16-bit number at `pos`; the caller has checked the bounds.
fn be16(bytes: &[u8], pos: usize) -> u32 {
    u32::from(bytes[pos]) << 8 | u32::from(bytes[pos + 1])
}

fn too_short(name: &str) -> String {
    format!("PGS {name} segment is too short")
}

/// The composition segment's body: 11 bytes, then 8 per object and 8 more for a cropped one.
fn composition(body: &[u8]) -> Result<(Composition, u8), String> {
    if body.len() < 11 {
        return Err(too_short("composition"));
    }
    let state = body[7];
    let count = usize::from(body[10]);
    let mut placements = Vec::with_capacity(count);
    let mut pos = 11;
    for _ in 0..count {
        if body.len() < pos + 8 {
            return Err(too_short("composition"));
        }
        let cropped = body[pos + 3] & CROPPED != 0;
        let (id, x, y) = (be16(body, pos) as u16, be16(body, pos + 4), be16(body, pos + 6));
        pos += 8;
        let crop = if cropped {
            if body.len() < pos + 8 {
                return Err(too_short("composition"));
            }
            let crop = [be16(body, pos), be16(body, pos + 2), be16(body, pos + 4), be16(body, pos + 6)];
            pos += 8;
            Some(crop)
        } else {
            None
        };
        placements.push(Placement { id, x, y, crop });
    }
    let video_width = be16(body, 0);
    let video_height = be16(body, 2);
    Ok((Composition { video_width, video_height, palette: body[9], placements }, state))
}

/// The window segment's body: a count, then 9 bytes per window; read for its length only (ffmpeg does not clip to
/// it).
fn window(body: &[u8]) -> Result<(), String> {
    let count = body.first().ok_or_else(|| too_short("window"))?;
    if body.len() < 1 + 9 * usize::from(*count) {
        return Err(too_short("window"));
    }
    Ok(())
}

impl PgsDecoder {
    /// A decoder with an empty epoch. Def: T5-4. Use: T5-8, T5-11.
    pub fn new() -> PgsDecoder {
        PgsDecoder::default()
    }

    /// Decodes one display set (design part 10.4); an `Err` holds one of the fixed texts below, which leaves the
    /// stream unread (design part 10.8). Def: T5-4. Use: T5-8, T5-11.
    pub fn decode(&mut self, display_set: &[u8]) -> Result<Bitmap, String> {
        let mut composition = None;
        let mut pos = 0;
        loop {
            if pos == display_set.len() {
                return Err("PGS display set has no end segment".to_string());
            }
            let cut = || "PGS display set ends inside a segment".to_string();
            if display_set.len() - pos < 3 {
                return Err(cut());
            }
            let kind = display_set[pos];
            let length = be16(display_set, pos + 1) as usize;
            let start = pos + 3;
            if display_set.len() - start < length {
                return Err(cut());
            }
            let body = &display_set[start..start + length];
            pos = start + length;
            match kind {
                COMPOSITION => {
                    if composition.is_some() {
                        return Err("PGS display set has two composition segments".to_string());
                    }
                    let (read, state) = self::composition(body)?;
                    if state & EPOCH_RESET != 0 {
                        self.palettes.clear();
                        self.objects.clear();
                        self.joining = None;
                    }
                    composition = Some(read);
                }
                WINDOW => window(body)?,
                PALETTE => self.palette(body)?,
                OBJECT => self.object(body)?,
                END => {
                    if pos != display_set.len() {
                        return Err("PGS display set has bytes after its end segment".to_string());
                    }
                    let composition = composition.ok_or("PGS display set has no composition segment")?;
                    return self.event(&composition);
                }
                other => return Err(format!("PGS segment type 0x{other:02x} is not known")),
            }
        }
    }

    /// A palette segment: palette id, version, then 5-byte entries of index, Y, Cr, Cb and alpha, each setting
    /// `[Y, alpha]` of its index; a palette first seen starts as 256 entries of `[0, 0]`.
    fn palette(&mut self, body: &[u8]) -> Result<(), String> {
        if body.len() < 2 || (body.len() - 2) % 5 != 0 {
            return Err(too_short("palette"));
        }
        let palette = self.palettes.entry(body[0]).or_insert_with(|| vec![[0, 0]; 256]);
        for entry in body[2..].chunks_exact(5) {
            palette[usize::from(entry[0])] = [entry[1], entry[4]];
        }
        Ok(())
    }

    /// An object segment: object id, version, sequence flags, then in a first segment the data length (counting the
    /// 4 bytes of width and height), width and height; then run-length data, decoded at the segment flagged last.
    fn object(&mut self, body: &[u8]) -> Result<(), String> {
        if body.len() < 4 {
            return Err(too_short("object"));
        }
        let id = be16(body, 0) as u16;
        let flags = body[3];
        let joining = if flags & FIRST != 0 {
            if body.len() < 11 {
                return Err(too_short("object"));
            }
            let declared = u32::from(body[4]) << 16 | be16(body, 5);
            // The declared length counts the 4 bytes of width and height the segment holds.
            let expected = declared.checked_sub(4).ok_or_else(|| too_short("object"))?;
            let data = body[11..].to_vec();
            Joining { id, expected, width: be16(body, 7), height: be16(body, 9), data }
        } else {
            match self.joining.take() {
                Some(mut joining) if joining.id == id => {
                    joining.data.extend_from_slice(&body[4..]);
                    joining
                }
                _ => return Err(format!("PGS object {id} continues without its first segment")),
            }
        };
        if flags & LAST == 0 {
            self.joining = Some(joining);
            return Ok(());
        }
        self.joining = None;
        let got = joining.data.len();
        if got as u64 != u64::from(joining.expected) {
            return Err(format!("PGS object {id} holds {got} bytes of run-length data, not {}", joining.expected));
        }
        let indices = decode_rle(id, &joining.data, joining.width, joining.height)?;
        self.objects.insert(id, Decoded { width: joining.width, height: joining.height, indices });
        Ok(())
    }

    /// The event at the end segment: the composition's objects in its order, each complete in the epoch, cropped
    /// when the composition crops it, with a copy of the composition's palette.
    fn event(&self, composition: &Composition) -> Result<Bitmap, String> {
        let mut objects = Vec::with_capacity(composition.placements.len());
        if !composition.placements.is_empty() {
            let id = composition.palette;
            let undefined = || format!("PGS palette {id} is not defined in its epoch");
            let palette = self.palettes.get(&id).ok_or_else(undefined)?;
            for placement in &composition.placements {
                let id = placement.id;
                let undefined = || format!("PGS object {id} is not defined in its epoch");
                let decoded = self.objects.get(&id).ok_or_else(undefined)?;
                let (width, height, indices) = match placement.crop {
                    None => (decoded.width, decoded.height, decoded.indices.clone()),
                    Some([cx, cy, cw, ch]) => {
                        let (ow, oh) = (decoded.width, decoded.height);
                        if cx + cw > ow || cy + ch > oh {
                            let place = format!("{cx},{cy} {cw} x {ch}");
                            return Err(format!("PGS object {id} crop {place} lies outside its {ow} x {oh}"));
                        }
                        let (cx, cy, cw, ch, ow) = (cx as usize, cy as usize, cw as usize, ch as usize, ow as usize);
                        // An object 0 pixels wide has no rows to take; its crop is then 0 wide too.
                        let rows = decoded.indices.chunks_exact(ow.max(1)).skip(cy).take(ch);
                        let indices = rows.flat_map(|row| &row[cx..cx + cw]).copied().collect();
                        (cw as u32, ch as u32, indices)
                    }
                };
                let (x, y) = (placement.x, placement.y);
                objects.push(Object { x, y, width, height, indices, palette: palette.clone() });
            }
        }
        Ok(Bitmap { video_width: composition.video_width, video_height: composition.video_height, objects })
    }
}

/// PGS run-length data (design part 10.4): a byte `C` other than 0 is one pixel of colour `C`; `0x00` then `b`
/// ends the line (`b == 0`) or is a run of `b & 0x3F` (or 14 bits with the next byte) pixels of colour 0 or of the
/// colour in the byte after. Every line holds exactly `width` pixels and ends at its end-of-line code, except the
/// last, which may end at the data's end; bytes after the `height`-th line are ignored.
fn decode_rle(id: u16, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let unfilled = || format!("PGS object {id} run-length data does not fill {width} x {height}");
    let mut indices = Vec::new();
    let mut lines = 0u32;
    let mut column = 0u32;
    let mut pos = 0usize;
    let next = |pos: &mut usize| {
        let byte = data.get(*pos).copied();
        *pos += 1;
        byte.ok_or_else(unfilled)
    };
    while lines < height {
        if pos >= data.len() {
            if lines + 1 == height && column == width {
                break;
            }
            return Err(unfilled());
        }
        let (run, colour) = match next(&mut pos)? {
            0 => match next(&mut pos)? {
                0 => {
                    if column != width {
                        return Err(unfilled());
                    }
                    lines += 1;
                    column = 0;
                    continue;
                }
                b => {
                    let short = u32::from(b & 0x3F);
                    let run = if b & 0x40 != 0 { short << 8 | u32::from(next(&mut pos)?) } else { short };
                    let colour = if b & 0x80 != 0 { next(&mut pos)? } else { 0 };
                    (run, colour)
                }
            },
            colour => (1, colour),
        };
        if run == 0 || run > width - column {
            return Err(unfilled());
        }
        indices.resize(indices.len() + run as usize, colour);
        column += run;
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::bitmap::{Bitmap, Object};
    use crate::testsupport::mks::pgs_display_set;

    const VIDEO: (u32, u32) = (1920, 1080);
    const END: [u8; 3] = [0x80, 0x00, 0x00];
    const WHITE: [u8; 4] = [235, 128, 128, 255];

    fn be16(v: u32) -> [u8; 2] {
        [(v >> 8) as u8, v as u8]
    }

    fn seg(kind: u8, body: &[u8]) -> Vec<u8> {
        let mut s = vec![kind];
        s.extend(be16(body.len() as u32));
        s.extend_from_slice(body);
        s
    }

    /// A composition segment; placements are (object id, x, y, crop as [x, y, width, height]).
    fn pcs(video: (u32, u32), state: u8, palette: u8, placements: &[(u16, u32, u32, Option<[u32; 4]>)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(be16(video.0));
        b.extend(be16(video.1));
        b.extend([0x10, 0x00, 0x01, state, 0x00, palette, placements.len() as u8]);
        for &(id, x, y, crop) in placements {
            b.extend(be16(id as u32));
            b.extend([0x00, if crop.is_some() { 0x80 } else { 0x00 }]);
            b.extend(be16(x));
            b.extend(be16(y));
            for v in crop.into_iter().flatten() {
                b.extend(be16(v));
            }
        }
        seg(0x16, &b)
    }

    fn pds(id: u8, entries: &[(u8, [u8; 4])]) -> Vec<u8> {
        let mut b = vec![id, 0x00];
        for (index, entry) in entries {
            b.push(*index);
            b.extend(entry);
        }
        seg(0x14, &b)
    }

    /// The object segments of one object, its run-length data split into `parts` segments.
    fn ods(id: u16, width: u32, height: u32, rle: &[u8], parts: usize) -> Vec<u8> {
        let chunk = rle.len().div_ceil(parts);
        let mut out = Vec::new();
        for (k, piece) in rle.chunks(chunk).enumerate() {
            let (first, last) = (k == 0, (k + 1) * chunk >= rle.len());
            let mut b = be16(id as u32).to_vec();
            b.extend([0x00, (if first { 0x80 } else { 0x00 }) | (if last { 0x40 } else { 0x00 })]);
            if first {
                let n = rle.len() as u32 + 4;
                b.extend([(n >> 16) as u8, (n >> 8) as u8, n as u8]);
                b.extend(be16(width));
                b.extend(be16(height));
            }
            b.extend_from_slice(piece);
            out.extend(seg(0x15, &b));
        }
        out
    }

    /// Rows of one colour each, in the long colour form.
    fn solid_rle(colour: u8, width: u32, height: u32) -> Vec<u8> {
        (0..height).flat_map(|_| [0x00, 0xC0 | (width >> 8) as u8, width as u8, colour, 0x00, 0x00]).collect()
    }

    fn palette_with(entries: &[(usize, [u8; 2])]) -> Vec<[u8; 2]> {
        let mut palette = vec![[0u8, 0u8]; 256];
        for &(i, e) in entries {
            palette[i] = e;
        }
        palette
    }

    fn err(text: &str) -> Result<Bitmap, String> {
        Err(text.to_string())
    }

    #[test]
    fn pgs_run_length_codes_decode_every_form() {
        let data = [
            0x05, 0x00, 0x03, 0x00, 0x40, 0x02, 0x00, 0x82, 0x07, 0x00, 0xC0, 0x03, 0x09, 0x00, 0x00, // line 0
            0x00, 0xC0, 0x0B, 0x01, 0x00, 0x00, // line 1
        ];
        let mut expected = vec![5, 0, 0, 0, 0, 0, 7, 7, 9, 9, 9];
        expected.extend([1; 11]);
        assert_eq!(decode_rle(0, &data, 11, 2), Ok(expected));
    }

    #[test]
    fn a_long_run_reads_fourteen_bits_of_length() {
        assert_eq!(decode_rle(0, &[0x00, 0x41, 0x2C, 0x00, 0x00], 300, 1), Ok(vec![0; 300]));
        assert_eq!(decode_rle(0, &[0x00, 0xFF, 0xFF, 0x04], 16383, 1), Ok(vec![4; 16383]));
    }

    #[test]
    fn run_length_data_that_does_not_fill_the_object_is_a_decoding_error() {
        let fails = |data: &[u8], w: u32, h: u32| {
            assert_eq!(decode_rle(3, data, w, h), Err(format!("PGS object 3 run-length data does not fill {w} x {h}")));
        };
        fails(&[0x05, 0x00, 0x00], 2, 1); // a line ended short
        fails(&[0x00, 0x83, 0x01], 2, 1); // a run past the line's end
        fails(&[0x01, 0x01, 0x00, 0x00], 2, 2); // too few lines
        fails(&[0x01, 0x01, 0x01, 0x01], 2, 2); // a line not ended by its code
        fails(&[0x00, 0x80, 0x05, 0x01, 0x01], 2, 1); // a run of length 0
        fails(&[0x00], 1, 1); // a code cut by the data's end
        assert_eq!(decode_rle(3, &[0x01, 0x01], 2, 1), Ok(vec![1, 1])); // the last line may end at the data's end
        assert_eq!(decode_rle(3, &[0x01, 0x00, 0x00, 0x07], 1, 1), Ok(vec![1])); // bytes after the last line
    }

    #[test]
    fn a_display_set_written_byte_by_byte_decodes_to_its_object() {
        let set: Vec<u8> = vec![
            0x16, 0x00, 0x13, 0x07, 0x80, 0x04, 0x38, 0x10, 0x00, 0x01, 0x80, 0x00, 0x00, 0x01, // composition
            0x00, 0x00, 0x00, 0x00, 0x00, 0x64, 0x03, 0xE8, // object 0 at 100, 1000
            0x17, 0x00, 0x0A, 0x01, 0x00, 0x00, 0x64, 0x03, 0xE8, 0x00, 0x02, 0x00, 0x02, // window
            0x14, 0x00, 0x07, 0x00, 0x00, 0x01, 0xEB, 0x80, 0x80, 0xFF, // palette 0, entry 1
            0x15, 0x00, 0x13, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x0C, 0x00, 0x02, 0x00, 0x02, // object 0, 2 x 2
            0x01, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, // its run-length data
            0x80, 0x00, 0x00, // end
        ];
        assert_eq!(pcs(VIDEO, 0x80, 0, &[(0, 100, 1000, None)]), set[0..22]);
        assert_eq!(pds(0, &[(1, WHITE)]), set[35..45]);
        assert_eq!(ods(0, 2, 2, &set[59..67], 1), set[45..67]);
        let object = Object { x: 100, y: 1000, width: 2, height: 2, indices: vec![1, 1, 0, 0], palette: palette_with(&[(1, [235, 255])]) };
        assert_eq!(PgsDecoder::new().decode(&set), Ok(Bitmap { video_width: 1920, video_height: 1080, objects: vec![object] }));
    }

    #[test]
    fn the_composition_size_is_the_bitmaps_video_size() {
        let set = [pcs((720, 576), 0x80, 0, &[(0, 8, 500, None)]), pds(0, &[(1, WHITE)]), ods(0, 4, 2, &solid_rle(1, 4, 2), 1), END.to_vec()].concat();
        let bitmap = PgsDecoder::new().decode(&set).unwrap();
        assert_eq!((bitmap.video_width, bitmap.video_height), (720, 576));
    }

    #[test]
    fn pgs_palette_entries_are_y_cr_cb_alpha() {
        let set = [pcs(VIDEO, 0x80, 0, &[(0, 0, 0, None)]), pds(0, &[(7, [200, 60, 90, 170])]), ods(0, 2, 1, &solid_rle(7, 2, 1), 1), END.to_vec()].concat();
        let palette = &PgsDecoder::new().decode(&set).unwrap().objects[0].palette;
        assert_eq!((palette.len(), palette[7], palette[8]), (256, [200, 170], [0, 0]));
    }

    #[test]
    fn the_composition_takes_its_palette_by_id_and_a_later_palette_segment_updates_only_its_entries() {
        let mut d = PgsDecoder::new();
        let object = ods(0, 2, 1, &[0x01, 0x02, 0x00, 0x00], 1);
        let first = [pcs(VIDEO, 0x80, 1, &[(0, 0, 0, None)]), pds(0, &[(1, [100, 128, 128, 255])]),
            pds(1, &[(1, [50, 128, 128, 255]), (2, [60, 128, 128, 128])]), object, END.to_vec()].concat();
        let a = d.decode(&first).unwrap();
        assert_eq!((a.objects[0].palette[1], a.objects[0].palette[2]), ([50, 255], [60, 128]));
        let update = [pcs(VIDEO, 0x00, 1, &[(0, 0, 0, None)]), pds(1, &[(1, [70, 128, 128, 255])]), END.to_vec()].concat();
        let b = d.decode(&update).unwrap();
        assert_eq!((b.objects[0].palette[1], b.objects[0].palette[2]), ([70, 255], [60, 128]));
        let missing = [pcs(VIDEO, 0x00, 2, &[(0, 0, 0, None)]), END.to_vec()].concat();
        assert_eq!(d.decode(&missing), err("PGS palette 2 is not defined in its epoch"));
    }

    #[test]
    fn a_later_display_set_reuses_an_object_of_its_epoch() {
        let mut d = PgsDecoder::new();
        let first = [pcs(VIDEO, 0x80, 0, &[(0, 100, 900, None)]), pds(0, &[(1, WHITE)]), ods(0, 4, 2, &solid_rle(1, 4, 2), 1), END.to_vec()].concat();
        let a = d.decode(&first).unwrap();
        let second = [pcs(VIDEO, 0x00, 0, &[(0, 300, 950, None)]), END.to_vec()].concat();
        let b = d.decode(&second).unwrap();
        assert_eq!((b.objects[0].x, b.objects[0].y), (300, 950));
        assert_eq!((&b.objects[0].indices, b.objects[0].palette[1]), (&a.objects[0].indices, [235, 255]));
    }

    #[test]
    fn a_new_epoch_or_acquisition_point_forgets_the_earlier_objects() {
        let first = [pcs(VIDEO, 0x80, 0, &[(0, 100, 900, None)]), pds(0, &[(1, WHITE)]), ods(0, 4, 2, &solid_rle(1, 4, 2), 1), END.to_vec()].concat();
        for state in [0x40u8, 0x80, 0xC0] {
            let mut d = PgsDecoder::new();
            d.decode(&first).unwrap();
            let again = [pcs(VIDEO, state, 0, &[(0, 300, 950, None)]), pds(0, &[(1, WHITE)]), END.to_vec()].concat();
            assert_eq!(d.decode(&again), err("PGS object 0 is not defined in its epoch"), "state {state:#04x}");
        }
    }

    #[test]
    fn an_object_split_over_segments_is_joined() {
        const RLE: [u8; 18] = [0x00, 0x83, 0x01, 0x00, 0x83, 0x02, 0x00, 0x00, 0x00, 0x86, 0x03, 0x00, 0x00, 0x00, 0x05, 0x04, 0x00, 0x00];
        let set = |parts: usize| [pcs(VIDEO, 0x80, 0, &[(5, 10, 20, None)]), pds(0, &[(1, WHITE)]), ods(5, 6, 3, &RLE, parts), END.to_vec()].concat();
        assert_eq!(set(3).len(), set(1).len() + 2 * 7);
        let split = PgsDecoder::new().decode(&set(3)).unwrap();
        assert_eq!(split.objects[0].indices, [1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 4]);
        assert_eq!(Ok(split), PgsDecoder::new().decode(&set(1)));
        let orphan = [pcs(VIDEO, 0x80, 0, &[]), seg(0x15, &[0x00, 0x02, 0x00, 0x40, 0x01, 0x00, 0x00]), END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&orphan), err("PGS object 2 continues without its first segment"));
    }

    #[test]
    fn an_object_whose_data_length_differs_from_its_declared_length_is_a_decoding_error() {
        let mut object = ods(0, 2, 1, &[0x01, 0x01, 0x00, 0x00], 1);
        object[9] += 1;
        let set = [pcs(VIDEO, 0x80, 0, &[(0, 0, 0, None)]), pds(0, &[(1, WHITE)]), object, END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&set), err("PGS object 0 holds 4 bytes of run-length data, not 5"));
    }

    #[test]
    fn the_compositions_cropping_is_applied() {
        let rle = [1, 2, 3, 4, 0, 0, 5, 6, 7, 8, 0, 0, 9, 10, 11, 12, 0, 0];
        let set = [pcs(VIDEO, 0x80, 0, &[(0, 50, 60, Some([1, 1, 2, 2]))]), pds(0, &[(1, WHITE)]), ods(0, 4, 3, &rle, 1), END.to_vec()].concat();
        let o = &PgsDecoder::new().decode(&set).unwrap().objects[0];
        assert_eq!((o.x, o.y, o.width, o.height, o.indices.clone()), (50, 60, 2, 2, vec![6, 7, 10, 11]));
    }

    #[test]
    fn a_crop_outside_its_object_is_a_decoding_error() {
        let set = [pcs(VIDEO, 0x80, 0, &[(0, 50, 60, Some([3, 2, 2, 2]))]), pds(0, &[(1, WHITE)]), ods(0, 4, 3, &solid_rle(1, 4, 3), 1), END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&set), err("PGS object 0 crop 3,2 2 x 2 lies outside its 4 x 3"));
    }

    #[test]
    fn a_composition_without_objects_decodes_to_no_objects() {
        let set = [pcs(VIDEO, 0x80, 0, &[]), END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&set), Ok(Bitmap { video_width: 1920, video_height: 1080, objects: vec![] }));
    }

    #[test]
    fn a_truncated_display_set_is_a_decoding_error() {
        let set = pgs_display_set(VIDEO, &[(0, 100, 900, 40, 20, 1)], &[(1, WHITE)]);
        assert!(PgsDecoder::new().decode(&set).is_ok());
        for cut in [1, 2, 10] {
            assert_eq!(PgsDecoder::new().decode(&set[..set.len() - cut]), err("PGS display set ends inside a segment"), "cut {cut}");
        }
        let short = [seg(0x16, &[0x07, 0x80]), END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&short), err("PGS composition segment is too short"));
        let palette = [pcs(VIDEO, 0x80, 0, &[]), seg(0x14, &[0x00, 0x00, 0x01, 0xEB]), END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&palette), err("PGS palette segment is too short"));
    }

    #[test]
    fn a_display_set_without_its_composition_or_end_or_with_an_unknown_segment_is_a_decoding_error() {
        let set = pgs_display_set(VIDEO, &[(0, 100, 900, 40, 20, 1)], &[(1, WHITE)]);
        assert_eq!(PgsDecoder::new().decode(&set[..set.len() - 3]), err("PGS display set has no end segment"));
        assert_eq!(PgsDecoder::new().decode(&[set.clone(), END.to_vec()].concat()), err("PGS display set has bytes after its end segment"));
        assert_eq!(PgsDecoder::new().decode(&[pds(0, &[(1, WHITE)]), END.to_vec()].concat()), err("PGS display set has no composition segment"));
        let two = [pcs(VIDEO, 0x80, 0, &[]), pcs(VIDEO, 0x00, 0, &[]), END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&two), err("PGS display set has two composition segments"));
        let unknown = [pcs(VIDEO, 0x80, 0, &[]), seg(0x33, &[0x01, 0x02]), END.to_vec()].concat();
        assert_eq!(PgsDecoder::new().decode(&unknown), err("PGS segment type 0x33 is not known"));
    }

    #[test]
    fn the_test_builders_display_set_decodes_to_its_rectangles() {
        let set = pgs_display_set(VIDEO, &[(0, 100, 900, 40, 20, 1), (1, 120, 950, 30, 10, 2)], &[(1, WHITE), (2, [16, 128, 128, 255])]);
        let b = PgsDecoder::new().decode(&set).unwrap();
        assert_eq!((b.video_width, b.video_height, b.objects.len()), (1920, 1080, 2));
        let (first, second) = (&b.objects[0], &b.objects[1]);
        assert_eq!((first.x, first.y, first.width, first.height, first.indices.clone()), (100, 900, 40, 20, vec![1; 800]));
        assert_eq!((second.x, second.y, second.width, second.height, second.indices.clone()), (120, 950, 30, 10, vec![2; 300]));
        assert_eq!((second.palette[1], second.palette[2]), ([235, 255], [16, 255]));
    }
}
