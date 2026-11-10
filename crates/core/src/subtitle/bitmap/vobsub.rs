//! The VobSub decoder of design part 10.4: one subpicture unit per block, its control sequences, its two interlaced
//! fields and the idx palette's colours.

use crate::subtitle::bitmap::mkv::IdxHeader;
use crate::subtitle::bitmap::{Bitmap, Object};

/// The command that ends a control sequence.
const END: u8 = 0xFF;

/// The values a unit's control sequences set, each replaced by a later sequence's.
#[derive(Debug, Default)]
struct Controls {
    /// The idx palette index of pixel codes 0 to 3.
    colours: Option<[u8; 4]>,
    /// The contrast nibble of pixel codes 0 to 3.
    alphas: Option<[u8; 4]>,
    /// `[x1, x2, y1, y2]`, inclusive.
    area: Option<[u32; 4]>,
    /// The top field's and the bottom field's offsets in the unit.
    fields: Option<[usize; 2]>,
}

/// A big-endian 16-bit number at `pos`; the caller has checked the bounds.
fn be16(bytes: &[u8], pos: usize) -> usize {
    usize::from(bytes[pos]) << 8 | usize::from(bytes[pos + 1])
}

/// The four nibbles of a colours or contrast command, from the first byte's high nibble the values of pixel codes 3,
/// 2, 1 and 0, as codes 0 to 3.
fn nibbles(bytes: &[u8]) -> [u8; 4] {
    [bytes[1] & 0x0F, bytes[1] >> 4, bytes[0] & 0x0F, bytes[0] >> 4]
}

/// The control sequence chain from the first sequence's offset: each a date (2), the next sequence's offset (2) and
/// commands up to `0xFF`. The chain is followed while a sequence's next offset is greater than its own and below the
/// unit's length.
fn controls(unit: &[u8]) -> Result<Controls, String> {
    let none = || "VobSub unit has no control sequence".to_string();
    if unit.len() < 4 {
        return Err(none());
    }
    let mut at = be16(unit, 2);
    // The first sequence must hold its date and next offset inside the unit; a later one whose header does not fit
    // ends the chain, as ffmpeg's dvdsubdec stops there.
    if at < 4 || unit.len() < at + 4 {
        return Err(none());
    }
    let mut found = Controls::default();
    loop {
        let next = be16(unit, at + 2);
        let mut pos = at + 4;
        loop {
            // A sequence cut before its 0xFF ends inside the end command it lacks.
            let unended = || format!("VobSub control sequence ends inside command 0x{END:02x}");
            let command = *unit.get(pos).ok_or_else(unended)?;
            pos += 1;
            let inside = |pos: usize, n: usize| {
                if unit.len() < pos + n {
                    Err(format!("VobSub control sequence ends inside command 0x{command:02x}"))
                } else {
                    Ok(&unit[pos..pos + n])
                }
            };
            match command {
                0x00..=0x02 => {}
                0x03 => {
                    found.colours = Some(nibbles(inside(pos, 2)?));
                    pos += 2;
                }
                0x04 => {
                    found.alphas = Some(nibbles(inside(pos, 2)?));
                    pos += 2;
                }
                0x05 => {
                    let b = inside(pos, 6)?.iter().map(|b| u32::from(*b)).collect::<Vec<u32>>();
                    let x1 = b[0] << 4 | b[1] >> 4;
                    let x2 = (b[1] & 0x0F) << 8 | b[2];
                    let y1 = b[3] << 4 | b[4] >> 4;
                    let y2 = (b[4] & 0x0F) << 8 | b[5];
                    found.area = Some([x1, x2, y1, y2]);
                    pos += 6;
                }
                0x06 => {
                    let b = inside(pos, 4)?;
                    found.fields = Some([be16(b, 0), be16(b, 2)]);
                    pos += 4;
                }
                0x07 => {
                    // Change colour and contrast: skipped by its length, which counts its own two bytes; not applied.
                    let length = be16(inside(pos, 2)?, 0);
                    inside(pos, length)?;
                    pos += length;
                }
                END => break,
                other => return Err(format!("VobSub control command 0x{other:02x} is not known")),
            }
        }
        if next <= at || next >= unit.len() || unit.len() < next + 4 {
            return Ok(found);
        }
        at = next;
    }
}

/// Decodes one VobSub subpicture unit against the track's idx header (design part 10.4); an `Err` holds one of the
/// fixed texts below, which leaves the stream unread (design part 10.8). Pure. Def: T5-4. Use: T5-8, T5-11.
pub fn decode_vobsub(unit: &[u8], idx: &IdxHeader) -> Result<Bitmap, String> {
    let found = controls(unit)?;
    let lacks = |what: &str| format!("VobSub control sequence lacks its {what}");
    let colours = found.colours.ok_or_else(|| lacks("colours"))?;
    let alphas = found.alphas.ok_or_else(|| lacks("contrast"))?;
    let [x1, x2, y1, y2] = found.area.ok_or_else(|| lacks("display area"))?;
    if x2 < x1 || y2 < y1 {
        return Err(format!("VobSub display area {x1},{y1} to {x2},{y2} is empty"));
    }
    let [top, bottom] = found.fields.ok_or_else(|| lacks("field offsets"))?;
    let (width, height) = (x2 - x1 + 1, y2 - y1 + 1);
    let top_rows = field_rows(unit, top, width, height.div_ceil(2))?;
    let bottom_rows = field_rows(unit, bottom, width, height / 2)?;
    let row = width as usize;
    let mut indices = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        let field = if y % 2 == 0 { &top_rows } else { &bottom_rows };
        indices.extend_from_slice(&field[y / 2 * row..(y / 2 + 1) * row]);
    }
    let palette = (0..4)
        .map(|c| {
            let alpha = vobsub_alpha(alphas[c]);
            [palette_luma(idx.palette[usize::from(colours[c])], alpha), alpha]
        })
        .collect();
    let object = Object { x: x1, y: y1, width, height, indices, palette };
    Ok(Bitmap { video_width: idx.width, video_height: idx.height, objects: vec![object] })
}

/// One field's rows of VobSub run-length data from byte `offset`: 4-bit nibbles, a value of one to four nibbles
/// whose low 2 bits are the pixel code and the rest the run, a run of 0 filling the rest of the row and a longer one
/// cut to it (as ffmpeg's dvdsubdec does); each row starts on a whole byte.
fn field_rows(unit: &[u8], offset: usize, width: u32, rows: u32) -> Result<Vec<u8>, String> {
    let ends = || format!("VobSub field data at offset {offset} ends before {width} x {rows}");
    // The position in nibbles, two to a byte, the high nibble first.
    let mut at = offset.checked_mul(2).ok_or_else(ends)?;
    let next = |at: &mut usize| {
        let byte = *unit.get(*at / 2).ok_or_else(ends)?;
        let value = if *at % 2 == 0 { byte >> 4 } else { byte & 0x0F };
        *at += 1;
        Ok::<u32, String>(u32::from(value))
    };
    let mut pixels = Vec::new();
    for _ in 0..rows {
        let mut column = 0u32;
        while column < width {
            let mut v = next(&mut at)?;
            if v < 0x4 {
                v = v << 4 | next(&mut at)?;
                if v < 0x10 {
                    v = v << 4 | next(&mut at)?;
                    if v < 0x40 {
                        v = v << 4 | next(&mut at)?;
                    }
                }
            }
            let rest = width - column;
            let run = match v >> 2 {
                0 => rest,
                run => run.min(rest),
            };
            pixels.resize(pixels.len() + run as usize, (v & 3) as u8);
            column += run;
        }
        // The next row starts on a whole byte.
        at += at % 2;
    }
    Ok(pixels)
}

/// swscale's video range BT.601 luma (ITU-R Recommendation BT.601) of an RGB colour, as FFmpeg 9.0.2's libswscale
/// converts RGB to the yuva444p that phase 0's chain gets before `mpdecimate`: `(8414 R + 16519 G + 3208 B +
/// 0x80100) >> 9` (input.c, `rgb16_32ToY_c_template`, with utils.c's default coefficients `(int)(c * 219 / 255 *
/// (1 << 15) + 0.5)` for c of 0.299, 0.587 and 0.114), doubled by the horizontal scaler (`hScale16To15_c`) and
/// rounded to 8 bits by `(y + 64) >> 7` (`yuv2plane1_8_c`); 16 for black, 235 for white.
fn video_luma(rgb: [u8; 3]) -> i32 {
    let [r, g, b] = rgb.map(i32::from);
    let y = (8414 * r + 16519 * g + 3208 * b + 0x80100) >> 9;
    ((2 * y).min(32767) + 64) >> 7
}

/// swscale's widening of an 8-bit video range luma to the full range of GRAY8 (FFmpeg 9.0.2, yuva444p to gray):
/// `y << 7` (`hScale8To15_c`), then `(y * 19078 - 39084288) >> 14` (`lumRangeToJpeg_c` with swscale.c's
/// `solve_range_convert` constants for 16 to 235 onto 0 to 255) and `(y + 64) >> 7`, clipped to 0 to 255.
fn full_range(luma: i32) -> u8 {
    let full = (((luma << 7) * 19078 - 39084288) >> 14).min(32767);
    ((full + 64) >> 7).clamp(0, 255) as u8
}

/// The grey phase 0's ffmpeg chain gives a pixel of an RGB colour at `alpha` (design part 10.4): the overlay over
/// the black canvas per channel, `((c * alpha + 128) * 257) >> 16` (vf_overlay's `FAST_DIV255`; the colour itself at
/// alpha 255, black at 0), its video range luma, `negate`'s `255 - Y` and the widening to full range. White text
/// gives 5 and the canvas 255.
fn phase0_grey(rgb: [u8; 3], alpha: u8) -> u8 {
    let a = u32::from(alpha);
    let blended = match alpha {
        255 => rgb,
        _ => rgb.map(|c| (((u32::from(c) * a + 128) * 257) >> 16) as u8),
    };
    full_range(255 - video_luma(blended))
}

/// The luma of an opaque RGB colour on `prepare`'s scale (design part 10.5): `255 - grey` of phase 0's ffmpeg grey of
/// the colour at alpha 255, so that `255 - Y` is ffmpeg's frame exactly; white gives 250, black 0. Def: T5-4. Use:
/// T5-4, T5-11.
pub fn bt601_luma(rgb: [u8; 3]) -> u8 {
    255 - phase0_grey(rgb, 255)
}

/// The Y a palette entry of an RGB colour and `alpha` holds (design part 10.4), the smallest for which `prepare`'s
/// `255 - round(Y * alpha / 255)` (design part 10.5) equals phase 0's ffmpeg grey of the colour at that alpha, which
/// one Y always reaches; ffmpeg's video range `negate` makes that grey no multiple of alpha, so Y depends on alpha.
/// At alpha 0 every Y gives white and the entry holds `bt601_luma`. Def: T5-4. Use: T5-4.
pub fn palette_luma(rgb: [u8; 3], alpha: u8) -> u8 {
    if alpha == 0 {
        return bt601_luma(rgb);
    }
    let (a, dark) = (i32::from(alpha), 255 - i32::from(phase0_grey(rgb, alpha)));
    let least = 255 * dark - 127;
    if least <= 0 { 0 } else { ((least + a - 1) / a).min(255) as u8 }
}

/// A VobSub contrast nibble on the 0 to 255 scale, `(alpha4 & 0x0F) * 17` (T5-R5; ffmpeg's dvdsubdec scales so).
/// Def: T5-4. Use: T5-4, T5-11.
pub fn vobsub_alpha(alpha4: u8) -> u8 {
    (alpha4 & 0x0F) * 17
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::bitmap::mkv::IdxHeader;
    use crate::subtitle::bitmap::{Bitmap, Object};
    use crate::testsupport::mks::vobsub_unit;

    /// Colours [0, 7, 9, 7] for pixel codes 0 to 3.
    const COLOURS: [u8; 3] = [0x03, 0x79, 0x70];
    /// Contrast [0, 15, 15, 8] for pixel codes 0 to 3.
    const CONTRAST: [u8; 3] = [0x04, 0x8F, 0xF0];
    /// x 10 to 13, y 20 to 22: 4 x 3 at 10, 20.
    const AREA: [u8; 7] = [0x05, 0x00, 0xA0, 0x0D, 0x01, 0x40, 0x16];
    /// The top field at offset 4, the bottom field at offset 6.
    const FIELDS: [u8; 5] = [0x06, 0x00, 0x04, 0x00, 0x06];
    const START: [u8; 1] = [0x01];
    const ALL: [&[u8]; 5] = [&COLOURS, &CONTRAST, &AREA, &FIELDS, &START];

    /// A unit whose rows 0 and 2 (top field) are codes 1 and 3 and whose row 1 (bottom field) is code 2, with one
    /// control sequence at offset 7 holding `commands`.
    fn unit_with(commands: &[&[u8]]) -> Vec<u8> {
        let mut u = vec![0x00, 0x00, 0x00, 0x07, 0x11, 0x13, 0x12, 0x00, 0x00, 0x00, 0x07];
        for command in commands {
            u.extend_from_slice(command);
        }
        u.push(0xFF);
        let n = u.len();
        u[0] = (n >> 8) as u8;
        u[1] = n as u8;
        u
    }

    fn idx(width: u32, height: u32) -> IdxHeader {
        let mut palette = [[0u8; 3]; 16];
        palette[7] = [255, 255, 255];
        palette[9] = [16, 16, 16];
        IdxHeader { width, height, palette }
    }

    fn err(text: &str) -> Result<Bitmap, String> {
        Err(text.to_string())
    }

    #[test]
    fn vobsub_run_length_codes_decode_every_form() {
        // Nibbles D | 1 6 | 0 4 3 | 0 1 0 1 | 0 0 0 0 (row 0), then 0 0 0 3 (row 1).
        let data = [0xD1, 0x60, 0x43, 0x01, 0x01, 0x00, 0x00, 0x00, 0x03];
        let mut row0 = vec![1u8; 3];
        row0.extend([2; 5]);
        row0.extend([3; 16]);
        row0.extend([1; 64]);
        row0.extend([0; 12]);
        assert_eq!(field_rows(&data, 0, 100, 2), Ok([row0, vec![3; 100]].concat()));
    }

    #[test]
    fn each_vobsub_row_starts_on_a_byte_and_a_long_run_stops_at_the_rows_end() {
        assert_eq!(field_rows(&[0xD0, 0xE0], 0, 3, 2), Ok(vec![1, 1, 1, 2, 2, 2]));
        assert_eq!(field_rows(&[0xD0], 0, 2, 1), Ok(vec![1, 1]));
        assert_eq!(field_rows(&[0x00, 0xD0], 1, 3, 1), Ok(vec![1, 1, 1]));
    }

    #[test]
    fn vobsub_field_data_that_ends_early_is_a_decoding_error() {
        assert_eq!(field_rows(&[0x40], 0, 4, 1), Err("VobSub field data at offset 0 ends before 4 x 1".to_string()));
    }

    #[test]
    fn vobsub_fields_are_interlaced() {
        let unit = unit_with(&ALL);
        assert_eq!(unit, [
            0x00, 0x1F, 0x00, 0x07, 0x11, 0x13, 0x12, 0x00, 0x00, 0x00, 0x07, 0x03, 0x79, 0x70, 0x04, 0x8F,
            0xF0, 0x05, 0x00, 0xA0, 0x0D, 0x01, 0x40, 0x16, 0x06, 0x00, 0x04, 0x00, 0x06, 0x01, 0xFF,
        ]);
        let o = &decode_vobsub(&unit, &idx(720, 480)).unwrap().objects[0];
        assert_eq!((o.x, o.y, o.width, o.height), (10, 20, 4, 3));
        assert_eq!(o.indices, [1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3]);
    }

    #[test]
    fn vobsub_colours_take_bt601_luma_from_the_idx_palette() {
        // ffmpeg's chain: RGB to video range luma, negate, widen to full range; white is 5 there, so its Y is 250.
        assert_eq!([bt601_luma([255, 255, 255]), bt601_luma([16, 16, 16]), bt601_luma([0, 0, 0])], [250, 12, 0]);
        assert_eq!([bt601_luma([255, 0, 0]), bt601_luma([0, 255, 0]), bt601_luma([0, 0, 255])], [71, 146, 24]);
        assert_eq!(bt601_luma([0, 90, 155]), 66);
        // White at alpha 136 is ffmpeg's grey 123 (overlay FAST_DIV255, then the chain above), which Y 247 gives.
        let o = &decode_vobsub(&unit_with(&ALL), &idx(720, 480)).unwrap().objects[0];
        assert_eq!(o.palette, [[0, 0], [250, 255], [12, 255], [247, 136]]);
    }

    #[test]
    fn a_palette_entrys_y_draws_ffmpegs_grey_at_its_alpha() {
        let drawn = |y: u8, alpha: u8| 255 - (u32::from(y) * u32::from(alpha) + 127) / 255;
        // ffmpeg's grey of white over black at alpha 17, 51, 128 and 255, through its video range negate.
        let greys = [17, 51, 128, 255].map(|alpha| phase0_grey([255, 255, 255], alpha));
        assert_eq!(greys, [242, 208, 132, 5]);
        for rgb in [[255, 255, 255], [16, 16, 16], [0, 90, 155], [200, 30, 70]] {
            for alpha in 0..=255u8 {
                assert_eq!(drawn(palette_luma(rgb, alpha), alpha), u32::from(phase0_grey(rgb, alpha)), "{rgb:?} at {alpha}");
            }
        }
    }

    #[test]
    fn vobsub_alpha_4_bit_scales_by_17() {
        assert_eq!([vobsub_alpha(0), vobsub_alpha(1), vobsub_alpha(8), vobsub_alpha(15)], [0, 17, 136, 255]);
        assert_eq!(vobsub_alpha(0x1F), 255);
    }

    #[test]
    fn the_idx_frame_size_is_the_vobsub_bitmaps_video_size() {
        let b = decode_vobsub(&unit_with(&ALL), &idx(720, 576)).unwrap();
        assert_eq!((b.video_width, b.video_height, b.objects.len()), (720, 576, 1));
    }

    #[test]
    fn a_second_control_sequence_keeps_the_values_of_the_first() {
        let one = unit_with(&ALL);
        let mut two = one.clone();
        two[0..2].copy_from_slice(&[0x00, 0x25]);
        two[9..11].copy_from_slice(&[0x00, 0x1F]);
        two.extend([0x00, 0x64, 0x00, 0x1F, 0x02, 0xFF]);
        assert_eq!(decode_vobsub(&two, &idx(720, 480)), decode_vobsub(&one, &idx(720, 480)));
    }

    #[test]
    fn a_vobsub_unit_without_a_control_sequence_is_a_decoding_error() {
        for offset in [[0x00, 0x40], [0x00, 0x02]] {
            let mut unit = unit_with(&ALL);
            unit[2..4].copy_from_slice(&offset);
            assert_eq!(decode_vobsub(&unit, &idx(720, 480)), err("VobSub unit has no control sequence"));
        }
        assert_eq!(decode_vobsub(&[0x00, 0x03, 0x00], &idx(720, 480)), err("VobSub unit has no control sequence"));
    }

    #[test]
    fn a_control_sequence_missing_a_value_or_with_an_unknown_command_is_a_decoding_error() {
        let fails = |commands: &[&[u8]], text: &str| assert_eq!(decode_vobsub(&unit_with(commands), &idx(720, 480)), err(text));
        fails(&[&CONTRAST, &AREA, &FIELDS, &START], "VobSub control sequence lacks its colours");
        fails(&[&COLOURS, &AREA, &FIELDS, &START], "VobSub control sequence lacks its contrast");
        fails(&[&COLOURS, &CONTRAST, &FIELDS, &START], "VobSub control sequence lacks its display area");
        fails(&[&COLOURS, &CONTRAST, &AREA, &START], "VobSub control sequence lacks its field offsets");
        fails(&[&COLOURS, &CONTRAST, &AREA, &FIELDS, &[0x09]], "VobSub control command 0x09 is not known");
        fails(&[&COLOURS, &CONTRAST, &FIELDS, &[0x05, 0x00, 0xA0]], "VobSub control sequence ends inside command 0x05");
        fails(&[&COLOURS, &CONTRAST, &[0x05, 0x00, 0xD0, 0x0A, 0x01, 0x40, 0x16], &FIELDS], "VobSub display area 13,20 to 10,22 is empty");
    }

    #[test]
    fn a_change_colour_command_is_skipped_by_its_length() {
        let skipped = unit_with(&[&COLOURS, &[0x07, 0x00, 0x06, 0xAA, 0xBB, 0xCC, 0xDD], &CONTRAST, &AREA, &FIELDS, &START]);
        assert_eq!(decode_vobsub(&skipped, &idx(720, 480)), decode_vobsub(&unit_with(&ALL), &idx(720, 480)));
    }

    #[test]
    fn the_test_builders_vobsub_unit_decodes_to_its_rectangle() {
        let b = decode_vobsub(&vobsub_unit((10, 20, 40, 30), 1, [0, 7, 9, 0], [0, 15, 15, 0]), &idx(720, 480)).unwrap();
        // ffmpeg's chain gives white a grey of 5 and RGB 16, 16, 16 one of 243, so their Y are 250 and 12.
        let object = Object { x: 10, y: 20, width: 40, height: 30, indices: vec![1; 1200], palette: vec![[0, 0], [250, 255], [12, 255], [0, 0]] };
        assert_eq!(b, Bitmap { video_width: 720, video_height: 480, objects: vec![object] });
    }
}
