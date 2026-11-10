//! The row line finder of design part 10.6 (P18; T5-R7): the inked row runs of a crop, the lines they group into,
//! each line's box, the median line height of a track and the four triggers that send an event to the fallback.
//! A change to any constant here after the trial of design part 10.9 needs a new trial (design 10.6, T5-R54).

use crate::subtitle::bitmap::prepare::{Crop, Grey, INK};

/// The gap of design part 10.6 (P18; T5-R7): 2 or more empty rows between two runs separate two lines, a single empty
/// row joins them. Def: T5-5. Use: T5-8.
pub const GAP_ROWS: u32 = 2;

/// The padding of design part 10.6 (P18; T5-R7) by which a line box grows on its left and right within the crop: 3
/// pixels, ocrs 0.13.1's own expansion of a detected word rectangle (`expand_dist` in its detection.rs); 16 until the
/// trial of 2026-11-07 showed the recognizer misreading line starts behind the longer white lead-in (the first glyph
/// doubled, dropped or replaced by `[`, `|`, `?` or `*`). Def: T5-5. Use: T5-8.
pub const LINE_PAD: u32 = 3;

/// The margin of design part 10.6 (P18; T5-R7) by which a line box grows above and below its inked rows within the
/// crop, at most half the empty rows to the neighbouring line (integer division) so that a neighbour's ink stays out:
/// 3 pixels like `LINE_PAD`, for the same reason. Def: T5-5. Use: T5-8.
pub const LINE_MARGIN: u32 = 3;

/// The factor of design part 10.6's triggers 1 and 2 (P18; T5-R7): a run or a line taller than 1.6 times the
/// stream's median line height is irregular. Def: T5-5. Use: T5-8.
pub const TALL_FACTOR: f64 = 1.6;

/// The most lines of design part 10.6's trigger 3 (P18; T5-R7): an event whose rows give more lines is irregular.
/// Def: T5-5. Use: T5-8.
pub const MAX_ROW_LINES: usize = 3;

/// The version of the bitmap decoders and their preparation (design parts 10.4, 10.5) that every OCR record names
/// (design part 10.12). Def: T5-5. Use: T5-6.
pub const DECODER_VERSION: &str = "lazymkv-bitmap 1";

/// The version of this line finder (design part 10.6, P18; T5-R7) that every OCR record names (design part 10.12).
/// Def: T5-5. Use: T5-6.
pub const LINE_FINDER_VERSION: &str = "lazymkv-rows 2";

/// One line box of design part 10.6 (P18; T5-R7) within the crop: the line's rows with `LINE_MARGIN` and its inked
/// columns padded by `LINE_PAD`. Def: T5-5. Use: T5-6, T5-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineBox {
    /// The box's left column within the crop. Def: T5-5. Use: T5-6, T5-8.
    pub x: u32,
    /// The box's top row within the crop. Def: T5-5. Use: T5-6, T5-8.
    pub y: u32,
    /// The box's width in pixels. Def: T5-5. Use: T5-6, T5-8.
    pub width: u32,
    /// The box's height in pixels. Def: T5-5. Use: T5-6, T5-8.
    pub height: u32,
}

/// The rows of a crop by design part 10.6 (P18; T5-R7): the maximal runs of inked rows and the lines they group into,
/// each `(first row, height)` within the crop, top to bottom. Def: T5-5. Use: T5-8, T5-10.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rows {
    /// The maximal blocks of contiguous inked rows. Def: T5-5. Use: T5-8, T5-10.
    pub runs: Vec<(u32, u32)>,
    /// The runs grouped across gaps of fewer than `GAP_ROWS` empty rows. Def: T5-5. Use: T5-8, T5-10.
    pub lines: Vec<(u32, u32)>,
}

/// The image's rows as slices, at most `height` of them; none for an image of no width.
fn image_rows(image: &Grey) -> impl Iterator<Item = &[u8]> {
    let width = (image.width as usize).max(1);
    let count = if image.width == 0 { 0 } else { image.height as usize };
    image.pixels.chunks_exact(width).take(count)
}

/// The rows of design part 10.6 (P18; T5-R7): a row is inked when it holds a pixel at or below `INK`; the runs are
/// the maximal blocks of inked rows, and consecutive runs separated by fewer than `GAP_ROWS` empty rows form one line.
/// Def: T5-5. Use: T5-8, T5-10.
pub fn rows(image: &Grey) -> Rows {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for (row, line) in image_rows(image).enumerate() {
        if !line.iter().any(|&p| p <= INK) {
            continue;
        }
        let row = row as u32;
        match runs.last_mut() {
            Some((first, height)) if *first + *height == row => *height += 1,
            _ => runs.push((row, 1)),
        }
    }
    let mut lines: Vec<(u32, u32)> = Vec::new();
    for &(first, height) in &runs {
        match lines.last_mut() {
            Some((start, span)) if first - (*start + *span) < GAP_ROWS => *span = first + height - *start,
            _ => lines.push((first, height)),
        }
    }
    Rows { runs, lines }
}

/// The line boxes of design part 10.6 (P18; T5-R7), one per line in `rows.lines` order: the line's rows grown by
/// `LINE_MARGIN` above and below, each side at most half the empty rows to the neighbouring line (to the crop's edge
/// where there is none), and the first to the last column holding a pixel at or below `INK` in the line's rows, padded
/// by `LINE_PAD` and clamped to the crop. A line with no such pixel in the image has no box. Def: T5-5. Use: T5-8,
/// T5-10.
pub fn line_boxes(image: &Grey, rows: &Rows) -> Vec<LineBox> {
    let all: Vec<&[u8]> = image_rows(image).collect();
    let mut boxes = Vec::with_capacity(rows.lines.len());
    for (index, &(first, height)) in rows.lines.iter().enumerate() {
        let start = (first as usize).min(all.len());
        let end = (first as usize).saturating_add(height as usize).min(all.len());
        let mut columns: Option<(usize, usize)> = None;
        for line in &all[start..end] {
            let (Some(left), Some(right)) = (line.iter().position(|&p| p <= INK), line.iter().rposition(|&p| p <= INK))
            else {
                continue;
            };
            columns = Some(columns.map_or((left, right), |(l, r)| (l.min(left), r.max(right))));
        }
        let Some((left, right)) = columns else {
            continue;
        };
        let gap_above = match index.checked_sub(1).map(|previous| rows.lines[previous]) {
            Some((prev_first, prev_height)) => first.saturating_sub(prev_first.saturating_add(prev_height)),
            None => first,
        };
        let bottom = first.saturating_add(height);
        let gap_below = match rows.lines.get(index + 1) {
            Some(&(next_first, _)) => next_first.saturating_sub(bottom),
            None => image.height.saturating_sub(bottom),
        };
        let above = LINE_MARGIN.min(gap_above / 2);
        let below = LINE_MARGIN.min(gap_below / 2);
        let x = (left as u32).saturating_sub(LINE_PAD);
        let last = (right as u32).saturating_add(LINE_PAD).min(image.width - 1);
        boxes.push(LineBox { x, y: first - above, width: last - x + 1, height: height + above + below });
    }
    boxes
}

/// The stream's median line height of design part 10.6's triggers 1 and 2 (P18; T5-R7): over every line of every
/// event with one or two lines, the middle height, or the mean of the two middle heights for an even count; `None`
/// when no event has one or two lines. Def: T5-5. Use: T5-8, T5-10.
pub fn median_line_height(events: &[Rows]) -> Option<f64> {
    let mut heights: Vec<u32> = events
        .iter()
        .filter(|event| matches!(event.lines.len(), 1 | 2))
        .flat_map(|event| event.lines.iter().map(|&(_, height)| height))
        .collect();
    if heights.is_empty() {
        return None;
    }
    heights.sort_unstable();
    let middle = heights.len() / 2;
    Some(if heights.len() % 2 == 1 {
        f64::from(heights[middle])
    } else {
        (f64::from(heights[middle - 1]) + f64::from(heights[middle])) / 2.0
    })
}

/// The first of design part 10.6's four triggers (P18; T5-R7) that holds, sending the event to ocrs's detector: 1, a
/// run taller than `TALL_FACTOR` times the median; 2, such a line; 3, more than `MAX_ROW_LINES` lines; 4, ink both
/// above and below the video frame's middle row `video_height / 2`, ink on that row being on neither side. Without
/// a median 1 and 2 never hold; `None` when no trigger holds. Def: T5-5. Use: T5-8, T5-10.
pub fn trigger(rows: &Rows, median: Option<f64>, crop: &Crop, video_height: u32) -> Option<u8> {
    if let Some(m) = median {
        let limit = TALL_FACTOR * m;
        if rows.runs.iter().any(|&(_, height)| f64::from(height) > limit) {
            return Some(1);
        }
        if rows.lines.iter().any(|&(_, height)| f64::from(height) > limit) {
            return Some(2);
        }
    }
    if rows.lines.len() > MAX_ROW_LINES {
        return Some(3);
    }
    let mid = u64::from(video_height / 2);
    let top = u64::from(crop.top);
    let runs = || rows.runs.iter().filter(|&&(_, height)| height > 0);
    let above = runs().any(|&(first, _)| top + u64::from(first) < mid);
    let below = runs().any(|&(first, height)| top + u64::from(first) + u64::from(height) - 1 > mid);
    if above && below {
        return Some(4);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::bitmap::prepare::{blocks, Crop, Grey, INK};

    const VIDEO: u32 = 1080; // the middle row is VIDEO / 2 = 540

    fn trigger_of(top: u32, image: Grey, median: Option<f64>) -> Option<u8> {
        let found = rows(&image);
        trigger(&found, median, &Crop { left: 0, top, image }, VIDEO)
    }

    fn heights(lines: &[u32]) -> Rows {
        let mut spans = Vec::new();
        let mut first = 0;
        for &height in lines {
            spans.push((first, height));
            first += height + GAP_ROWS;
        }
        Rows { runs: spans.clone(), lines: spans }
    }

    #[test]
    fn one_empty_row_does_not_split_a_line_and_two_do() {
        let found = rows(&blocks(40, 30, &[(5, 5, 10, 5), (5, 11, 10, 4), (5, 17, 10, 4)]));
        assert_eq!(found.runs, [(5, 5), (11, 4), (17, 4)]);
        assert_eq!(found.lines, [(5, 10), (17, 4)], "row 10 joins; rows 15 and 16 separate");
        assert_eq!(rows(&blocks(40, 30, &[])), Rows::default());
    }

    #[test]
    fn a_row_with_one_pixel_at_127_is_inked() {
        let mut image = blocks(40, 30, &[]);
        image.pixels[25 * 40 + 39] = INK + 1;
        assert_eq!(rows(&image), Rows::default());
        image.pixels[25 * 40 + 39] = INK;
        assert_eq!(rows(&image).lines, [(25, 1)]);
    }

    #[test]
    fn line_boxes_span_inked_columns_padded_by_3_with_a_margin_of_at_most_3_rows() {
        let image = blocks(100, 40, &[(30, 5, 10, 5), (50, 11, 10, 4), (5, 17, 90, 4)]);
        let boxes = line_boxes(&image, &rows(&image));
        assert_eq!(
            boxes,
            [LineBox { x: 27, y: 3, width: 36, height: 13 }, LineBox { x: 2, y: 16, width: 96, height: 8 }],
            "line 1: above min(3, 5 / 2) = 2, below min(3, 2 / 2) = 1; line 2: above 1, below min(3, 19 / 2) = 3"
        );
    }

    #[test]
    fn a_margin_stops_at_the_crops_edges() {
        let top = blocks(20, 20, &[(0, 0, 20, 10)]);
        assert_eq!(line_boxes(&top, &rows(&top)), [LineBox { x: 0, y: 0, width: 20, height: 13 }]);
        let bottom = blocks(20, 20, &[(0, 10, 20, 10)]);
        assert_eq!(line_boxes(&bottom, &rows(&bottom)), [LineBox { x: 0, y: 7, width: 20, height: 13 }]);
    }

    #[test]
    fn two_lines_two_rows_apart_never_share_a_row() {
        let image = blocks(30, 40, &[(0, 10, 30, 8), (0, 20, 30, 8)]);
        let boxes = line_boxes(&image, &rows(&image));
        assert_eq!(boxes.len(), 2);
        assert_eq!((boxes[0].y, boxes[0].height), (7, 12), "above 3, below 1");
        assert_eq!((boxes[1].y, boxes[1].height), (19, 12), "above 1, below 3");
        assert!(boxes[0].y + boxes[0].height <= boxes[1].y);
    }

    #[test]
    fn the_median_counts_only_one_and_two_line_events() {
        let events = [heights(&[30]), heights(&[32, 34]), heights(&[100, 100, 100]), Rows::default()];
        assert_eq!(median_line_height(&events), Some(32.0));
        let even = [heights(&[30]), heights(&[32, 34]), heights(&[40])];
        assert_eq!(median_line_height(&even), Some(33.0), "an even count takes the mean of the middle two");
        assert_eq!(median_line_height(&[heights(&[100, 100, 100]), Rows::default()]), None);
        assert_eq!(median_line_height(&[]), None);
    }

    #[test]
    fn a_run_taller_than_1_6_medians_is_trigger_1() {
        assert_eq!(trigger_of(900, blocks(30, 40, &[(0, 0, 30, 33)]), Some(20.0)), Some(1));
        assert_eq!(trigger_of(900, blocks(30, 40, &[(0, 0, 30, 32)]), Some(20.0)), None, "32 rows is not taller than 1.6 x 20");
    }

    #[test]
    fn a_line_taller_than_1_6_medians_whose_runs_are_not_is_trigger_2() {
        let image = blocks(30, 45, &[(0, 0, 30, 20), (0, 21, 30, 20)]);
        assert_eq!(rows(&image).lines, [(0, 41)]);
        assert_eq!(trigger_of(900, image, Some(20.0)), Some(2));
    }

    #[test]
    fn four_lines_are_trigger_3_and_three_are_not() {
        let bars = [(0, 0, 30, 10), (0, 13, 30, 10), (0, 26, 30, 10), (0, 39, 30, 10)];
        assert_eq!(trigger_of(900, blocks(30, 60, &bars), Some(10.0)), Some(3));
        assert_eq!(trigger_of(900, blocks(30, 60, &bars[..3]), Some(10.0)), None);
    }

    #[test]
    fn ink_above_and_below_the_frames_middle_row_is_trigger_4() {
        assert_eq!(trigger_of(500, blocks(30, 100, &[(0, 0, 30, 10), (0, 80, 30, 10)]), Some(10.0)), Some(4));
        assert_eq!(trigger_of(500, blocks(30, 100, &[(0, 30, 30, 12)]), None), Some(4), "frame rows 530 to 541 cross row 540");
        assert_eq!(trigger_of(500, blocks(30, 100, &[(0, 31, 30, 10)]), None), None, "531 to 540 ends on the middle row");
        assert_eq!(trigger_of(540, blocks(30, 100, &[(0, 0, 30, 10)]), None), None, "540 to 549 starts on it");
    }

    #[test]
    fn a_regular_event_has_no_trigger() {
        let image = blocks(600, 100, &[(16, 16, 568, 30), (16, 56, 568, 30)]);
        assert_eq!(rows(&image).lines.len(), 2);
        assert_eq!(trigger_of(900, image, Some(30.0)), None);
    }

    #[test]
    fn without_a_median_triggers_1_and_2_never_hold() {
        assert_eq!(trigger_of(560, blocks(30, 500, &[(0, 0, 30, 500)]), None), None);
        assert_eq!(trigger_of(560, blocks(30, 500, &[(0, 0, 30, 500)]), Some(10.0)), Some(1));
    }

    #[test]
    fn the_first_trigger_that_holds_is_named() {
        let image = || blocks(30, 200, &[(0, 0, 30, 40), (0, 50, 30, 10), (0, 70, 30, 10), (0, 150, 30, 10)]);
        assert_eq!(trigger_of(450, image(), Some(10.0)), Some(1), "1, 3 and 4 hold");
        assert_eq!(trigger_of(450, image(), None), Some(3), "3 and 4 hold");
    }

    #[test]
    fn the_line_finders_constants_are_design_10_6s() {
        assert_eq!((INK, GAP_ROWS, LINE_PAD, LINE_MARGIN, TALL_FACTOR, MAX_ROW_LINES), (127, 2, 3, 3, 1.6, 3));
        assert_eq!((DECODER_VERSION, LINE_FINDER_VERSION), ("lazymkv-bitmap 1", "lazymkv-rows 2"));
    }
}
