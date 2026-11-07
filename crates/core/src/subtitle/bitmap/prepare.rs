//! The preparation of design part 10.5 ([OCR-13], [OCR-9]; T5-R6): a decoded event drawn as grey on the white canvas
//! of the video frame, the distinct crop around its dark pixels and the crop's key.

use sha1::{Digest, Sha1};

use crate::subtitle::bitmap::Bitmap;

/// The ink level of design part 10.5 ([OCR-9]): a grey pixel at or below 127 is dark, part of the text; the crop and
/// the line finder of design part 10.6 both use it. Def: T5-5. Use: T5-5, T5-8.
pub const INK: u8 = 127;

/// The padding of design part 10.5 ([OCR-13]; T5-R6) by which the crop's box grows on every side before it is
/// clamped to the frame. Def: T5-5. Use: T5-8.
pub const CROP_PAD: u32 = 16;

/// A grey image of design part 10.5: `width * height` pixels, row by row from the top, 0 black to 255 white.
/// Def: T5-5. Use: T5-8, T5-10, T5-11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grey {
    /// The image's width in pixels. Def: T5-5. Use: T5-8, T5-10, T5-11.
    pub width: u32,
    /// The image's height in pixels. Def: T5-5. Use: T5-8, T5-10, T5-11.
    pub height: u32,
    /// The grey of each pixel, row by row; `width * height` of them. Def: T5-5. Use: T5-8, T5-10, T5-11.
    pub pixels: Vec<u8>,
}

/// The distinct crop of design part 10.5 ([OCR-13]; T5-R6): the dark box of a prepared frame grown by `CROP_PAD`
/// and clamped to the frame, with its top-left corner on the video frame. Def: T5-5. Use: T5-8, T5-10, T5-11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crop {
    /// The crop's left column on the video frame. Def: T5-5. Use: T5-8, T5-10, T5-11.
    pub left: u32,
    /// The crop's top row on the video frame. Def: T5-5. Use: T5-8, T5-10, T5-11.
    pub top: u32,
    /// The frame's pixels inside the crop's box. Def: T5-5. Use: T5-8, T5-10, T5-11.
    pub image: Grey,
}

/// The preparation of design part 10.5 ([OCR-9]; T5-R6, R2-decoders-34): the event's objects drawn on a white
/// canvas of the video frame's size, each pixel `255 - round(Y * alpha / 255)` of its palette entry, the darker grey
/// kept where objects overlap; a pixel outside the frame is clipped and an index without a palette entry is
/// transparent. Def: T5-5. Use: T5-8, T5-11.
pub fn prepare(bitmap: &Bitmap) -> Grey {
    let (width, height) = (bitmap.video_width, bitmap.video_height);
    let mut pixels = vec![255u8; (width as usize) * (height as usize)];
    for object in &bitmap.objects {
        for j in 0..object.height {
            let Some(row) = object.y.checked_add(j).filter(|&row| row < height) else {
                continue;
            };
            for i in 0..object.width {
                let Some(column) = object.x.checked_add(i).filter(|&column| column < width) else {
                    continue;
                };
                let at = (j as usize) * (object.width as usize) + i as usize;
                let Some(&[y, alpha]) = object.indices.get(at).and_then(|&index| object.palette.get(usize::from(index)))
                else {
                    continue;
                };
                let grey = 255 - (u32::from(y) * u32::from(alpha) + 127) / 255;
                let pixel = &mut pixels[(row as usize) * (width as usize) + column as usize];
                *pixel = (*pixel).min(grey as u8);
            }
        }
    }
    Grey { width, height, pixels }
}

/// The distinct crop of design part 10.5 ([OCR-13]; T5-R6): the box of every pixel at or below `INK`, grown by
/// `CROP_PAD` on every side and clamped to the frame, with a copy of the frame's pixels in it; `None` when no pixel is
/// dark. Def: T5-5. Use: T5-8, T5-10, T5-11.
pub fn crop(frame: &Grey) -> Option<Crop> {
    let width = frame.width as usize;
    if width == 0 || frame.pixels.len() < width * frame.height as usize {
        return None;
    }
    let mut dark: Option<[u32; 4]> = None;
    for (row, line) in frame.pixels.chunks_exact(width).take(frame.height as usize).enumerate() {
        let (Some(first), Some(last)) = (line.iter().position(|&p| p <= INK), line.iter().rposition(|&p| p <= INK))
        else {
            continue;
        };
        let (row, first, last) = (row as u32, first as u32, last as u32);
        dark = Some(match dark {
            None => [first, row, last, row],
            Some([x0, y0, x1, _]) => [x0.min(first), y0, x1.max(last), row],
        });
    }
    let [x0, y0, x1, y1] = dark?;
    let left = x0.saturating_sub(CROP_PAD);
    let top = y0.saturating_sub(CROP_PAD);
    let right = x1.saturating_add(CROP_PAD).min(frame.width - 1);
    let bottom = y1.saturating_add(CROP_PAD).min(frame.height - 1);
    let mut pixels = Vec::with_capacity(((right - left + 1) as usize) * ((bottom - top + 1) as usize));
    for row in top..=bottom {
        let start = (row as usize) * width;
        pixels.extend_from_slice(&frame.pixels[start + left as usize..=start + right as usize]);
    }
    Some(Crop { left, top, image: Grey { width: right - left + 1, height: bottom - top + 1, pixels } })
}

/// The crop's key of design part 10.5 (R2-decoders-32): SHA-1 over the width and the height as `u32` little-endian
/// and then the pixels, used only to find identical bitmaps; the crop's place on the frame is not in it.
/// Def: T5-5. Use: T5-8.
pub fn crop_key(image: &Grey) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(image.width.to_le_bytes());
    hasher.update(image.height.to_le_bytes());
    hasher.update(&image.pixels);
    let mut key = [0u8; 20];
    key.copy_from_slice(&hasher.finalize());
    key
}

/// A test frame of `width` x `height` white pixels with each `(x, y, w, h)` rectangle of `dark` black; the rectangles
/// lie inside the frame.
#[cfg(test)]
pub(crate) fn blocks(width: u32, height: u32, dark: &[(u32, u32, u32, u32)]) -> Grey {
    let mut pixels = vec![255u8; (width as usize) * (height as usize)];
    for &(x, y, w, h) in dark {
        for row in y..y + h {
            for column in x..x + w {
                pixels[(row as usize) * (width as usize) + column as usize] = 0;
            }
        }
    }
    Grey { width, height, pixels }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::bitmap::{Bitmap, Object};

    fn bitmap(video: (u32, u32), objects: Vec<Object>) -> Bitmap {
        Bitmap { video_width: video.0, video_height: video.1, objects }
    }

    fn object(at: (u32, u32), size: (u32, u32), indices: Vec<u8>, palette: Vec<[u8; 2]>) -> Object {
        Object { x: at.0, y: at.1, width: size.0, height: size.1, indices, palette }
    }

    fn shape(c: &Crop) -> (u32, u32, u32, u32) {
        (c.left, c.top, c.image.width, c.image.height)
    }

    fn pixel(c: &Crop, x: u32, y: u32) -> u8 {
        c.image.pixels[(y * c.image.width + x) as usize]
    }

    fn hex(key: &[u8; 20]) -> String {
        key.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn prepare_turns_opaque_white_dark_and_black_outlines_white() {
        // [Y, alpha]: opaque white text, an opaque black outline, transparent white, half-transparent light grey.
        let palette = vec![[235, 255], [16, 255], [235, 0], [200, 128]];
        let frame = prepare(&bitmap((8, 4), vec![object((2, 1), (4, 1), vec![0, 1, 2, 3], palette)]));
        assert_eq!((frame.width, frame.height, frame.pixels.len()), (8, 4, 32));
        assert_eq!(&frame.pixels[10..14], &[20, 239, 255, 155], "255 - round(Y * alpha / 255)");
        for (i, &v) in frame.pixels.iter().enumerate() {
            if !(10..14).contains(&i) {
                assert_eq!(v, 255, "pixel {i} is the white canvas");
            }
        }
    }

    #[test]
    fn the_canvas_is_the_video_frame_and_an_object_beyond_it_is_clipped() {
        let frame = prepare(&bitmap((4, 3), vec![object((2, 1), (4, 3), vec![0; 12], vec![[255, 255]])]));
        assert_eq!((frame.width, frame.height), (4, 3));
        assert_eq!(frame.pixels, [255, 255, 255, 255, 255, 255, 0, 0, 255, 255, 0, 0]);
    }

    #[test]
    fn an_index_without_a_palette_entry_is_transparent() {
        let frame = prepare(&bitmap((3, 2), vec![
            object((0, 0), (2, 1), vec![0, 5], vec![[255, 255]]),
            object((0, 1), (3, 1), vec![0], vec![[255, 255]]),
        ]));
        assert_eq!(frame.pixels, [0, 255, 255, 0, 255, 255], "index 5 has no entry; the second object's indices stop after one");
    }

    #[test]
    fn where_two_objects_overlap_the_darker_grey_is_kept() {
        let dark = || object((0, 0), (2, 1), vec![0, 0], vec![[255, 255]]);
        let light = || object((1, 0), (2, 1), vec![0, 0], vec![[100, 255]]);
        assert_eq!(prepare(&bitmap((3, 1), vec![dark(), light()])).pixels, [0, 0, 155]);
        assert_eq!(prepare(&bitmap((3, 1), vec![light(), dark()])).pixels, [0, 0, 155], "the order does not matter");
    }

    #[test]
    fn the_crop_is_the_dark_box_grown_by_16_and_clamped_to_the_frame() {
        let middle = crop(&blocks(100, 50, &[(40, 20, 10, 5)])).unwrap();
        assert_eq!(shape(&middle), (24, 4, 42, 37));
        assert_eq!((pixel(&middle, 16, 16), pixel(&middle, 15, 16), pixel(&middle, 25, 20)), (0, 255, 0));
        let top_left = crop(&blocks(100, 50, &[(10, 5, 10, 5)])).unwrap();
        assert_eq!(shape(&top_left), (0, 0, 36, 26), "T5-R6: clamped to the frame, not grown past it");
        let bottom_right = crop(&blocks(100, 50, &[(95, 45, 5, 5)])).unwrap();
        assert_eq!(shape(&bottom_right), (79, 29, 21, 21));
        assert_eq!(pixel(&bottom_right, 16, 16), 0);
        let two = crop(&blocks(100, 50, &[(30, 10, 2, 2), (60, 30, 2, 2)])).unwrap();
        assert_eq!(shape(&two), (14, 0, 64, 48), "one box around every dark pixel");
    }

    #[test]
    fn grey_127_is_ink_and_128_is_not() {
        let mut frame = blocks(40, 40, &[]);
        frame.pixels[20 * 40 + 20] = 128;
        assert_eq!(crop(&frame), None);
        frame.pixels[20 * 40 + 20] = 127;
        assert_eq!(shape(&crop(&frame).unwrap()), (4, 4, 33, 33));
        assert_eq!((INK, CROP_PAD), (127, 16));
    }

    #[test]
    fn a_frame_without_dark_pixels_has_no_crop() {
        assert_eq!(crop(&blocks(20, 10, &[])), None);
        assert_eq!(crop(&Grey { width: 0, height: 0, pixels: vec![] }), None);
        assert_eq!(prepare(&bitmap((20, 10), vec![])), blocks(20, 10, &[]));
    }

    #[test]
    fn identical_crops_share_a_key_and_a_one_pixel_difference_does_not() {
        let a = crop(&blocks(100, 50, &[(40, 20, 10, 5)])).unwrap().image;
        let moved = crop(&blocks(100, 50, &[(50, 25, 10, 5)])).unwrap().image;
        assert_eq!(crop_key(&a), crop_key(&moved), "a repeat at another place on the frame is the same crop");
        let mut changed = a.clone();
        changed.pixels[0] = 254;
        assert_ne!(crop_key(&a), crop_key(&changed));
    }

    #[test]
    fn the_key_is_sha1_over_the_size_then_the_pixels() {
        // SHA-1 of 01 00 00 00, 01 00 00 00, ff: width and height as u32 little-endian, then the one pixel.
        assert_eq!(hex(&crop_key(&Grey { width: 1, height: 1, pixels: vec![255] })), "ec6553cfe407ced4c06597fa1abe348b11f2f8ed");
        let wide = Grey { width: 3, height: 2, pixels: vec![0; 6] };
        let tall = Grey { width: 2, height: 3, pixels: vec![0; 6] };
        assert_ne!(crop_key(&wide), crop_key(&tall), "the same pixels in another shape are another crop");
    }
}
