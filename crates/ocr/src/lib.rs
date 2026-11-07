//! lazymkv-ocr: the OCR (optical character recognition) worker lazymkv-ocr.exe over ocrs and rten, the only package
//! that links them; the engine reaches it only by starting the process (design parts 10.1 and 10.7; notes A2, call 23;
//! K1).

pub mod stream;

use std::io::Cursor;
use std::time::Instant;

use lazymkv_core::subtitle::bitmap::evidence::EngineId;
use lazymkv_core::subtitle::bitmap::lines::LineBox;
use lazymkv_core::subtitle::bitmap::prepare::Grey;
use lazymkv_ocr_models::{DETECTION, DETECTION_SHA256, RECOGNITION, RECOGNITION_SHA256};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams, OcrInput, TextLine};
use rten::Model;
use rten_imageproc::{bounding_rect, RectF, RotatedRect};

/// The ocrs version this worker is built with, the workspace's `=0.13.1` pin (design part 10.7, [ARCH-13]); the two
/// change together, and `engine` in every record names it (design part 10.12). Def: T5-8. Use: T5-8.
pub const OCRS_VERSION: &str = "0.13.1";

/// ocrs's engine over the two embedded models, loaded once per worker process (design part 10.7, K2). Def: T5-8.
/// Use: T5-8, T5-10.
pub struct Ocr {
    engine: OcrEngine,
}

/// The detector's line boxes and lines of one crop, with the two stage times the record's `ms.detect` and
/// `ms.recognize` take (design parts 10.6, 10.12). Def: T5-8. Use: T5-8, T5-10.
#[derive(Clone, Debug, PartialEq)]
pub struct Detected {
    /// One box per line ocrs found, in its line order: the union of the line's word rectangles.
    pub boxes: Vec<LineBox>,
    /// The recognised lines, each trimmed, empty ones left out; never joined (design part 10.12).
    pub lines: Vec<String>,
    /// The milliseconds of `prepare_input`, `detect_words` and `find_text_lines`.
    pub detect_ms: f64,
    /// The milliseconds of `recognize_text`.
    pub recognize_ms: f64,
}

/// The milliseconds since `start`.
fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// The kept lines of a recognition, phase 0's rule: each `Some` line trimmed, `None` and empty lines left out.
fn kept_lines(read: Vec<Option<TextLine>>) -> Vec<String> {
    read.into_iter()
        .flatten()
        .map(|line| line.to_string().trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// A box's rectangle as a single-rectangle line of ocrs.
fn line_rect(b: &LineBox) -> Vec<RotatedRect> {
    vec![RotatedRect::from_rect(RectF::from_tlhw(b.y as f32, b.x as f32, b.height as f32, b.width as f32))]
}

/// The union of a found line's word rectangles as a box: left and top floored and clamped at 0, right and bottom
/// ceiled and clamped to the image.
fn word_union(line: &[RotatedRect], image: &Grey) -> Option<LineBox> {
    let rect = bounding_rect(line.iter())?;
    let (w, h) = (image.width as f32, image.height as f32);
    let left = rect.left().floor().clamp(0.0, w);
    let top = rect.top().floor().clamp(0.0, h);
    let right = rect.right().ceil().clamp(left, w);
    let bottom = rect.bottom().ceil().clamp(top, h);
    let (x, y) = (left as u32, top as u32);
    Some(LineBox { x, y, width: right as u32 - x, height: bottom as u32 - y })
}

impl Ocr {
    /// Loads both embedded models with rten's `Model::load_static_slice` into one ocrs engine (design part 10.7);
    /// Err is `the OCR models did not load: ` and the error. Def: T5-8. Use: T5-8, T5-10.
    pub fn load() -> Result<Ocr, String> {
        let failed = |error: String| format!("the OCR models did not load: {error}");
        let detection = Model::load_static_slice(DETECTION).map_err(|e| failed(e.to_string()))?;
        let recognition = Model::load_static_slice(RECOGNITION).map_err(|e| failed(e.to_string()))?;
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection),
            recognition_model: Some(recognition),
            ..Default::default()
        })
        .map_err(|e| failed(e.to_string()))?;
        Ok(Ocr { engine })
    }

    /// The image as ocrs sees it: each grey value three times as RGB bytes, through `ImageSource::from_bytes` and
    /// `prepare_input`, the form phase 0's trial handed ocrs (K2).
    fn input(&self, image: &Grey) -> Result<OcrInput, String> {
        let rgb: Vec<u8> = image.pixels.iter().flat_map(|&p| [p, p, p]).collect();
        let source = ImageSource::from_bytes(&rgb, (image.width, image.height)).map_err(|e| e.to_string())?;
        self.engine.prepare_input(source).map_err(|e| e.to_string())
    }

    /// Reads the line boxes of one crop with ocrs's `recognize_text`, each box given as one rectangle, in the boxes'
    /// order, which is top to bottom (design part 10.7). Def: T5-8. Use: T5-8, T5-10.
    pub fn recognize(&self, image: &Grey, boxes: &[LineBox]) -> Result<Vec<String>, String> {
        if boxes.is_empty() {
            return Ok(Vec::new());
        }
        let input = self.input(image)?;
        let lines: Vec<Vec<RotatedRect>> = boxes.iter().map(line_rect).collect();
        let read = self.engine.recognize_text(&input, &lines).map_err(|e| e.to_string())?;
        Ok(kept_lines(read))
    }

    /// Reads an irregular crop through ocrs's detector: `detect_words`, `find_text_lines`, then `recognize_text`
    /// over the found lines (design part 10.6, P18). Def: T5-8. Use: T5-8, T5-10.
    pub fn detect(&self, image: &Grey) -> Result<Detected, String> {
        let start = Instant::now();
        let input = self.input(image)?;
        let words = self.engine.detect_words(&input).map_err(|e| e.to_string())?;
        let found = self.engine.find_text_lines(&input, &words);
        let detect_ms = ms_since(start);
        let start = Instant::now();
        let read = self.engine.recognize_text(&input, &found).map_err(|e| e.to_string())?;
        let recognize_ms = ms_since(start);
        // find_text_lines gives no line without a word, so every found line has a box.
        let boxes = found.iter().filter_map(|line| word_union(line, image)).collect();
        Ok(Detected { boxes, lines: kept_lines(read), detect_ms, recognize_ms })
    }
}

/// The `engine` of every record and of `run.json`: ocrs's version and both models' SHA-256 (Secure Hash Algorithm
/// 256) pins (design part 10.12). Def: T5-8. Use: T5-8, T5-10.
pub fn engine_id() -> EngineId {
    EngineId {
        ocrs: OCRS_VERSION.to_string(),
        detection_sha256: DETECTION_SHA256.to_string(),
        recognition_sha256: RECOGNITION_SHA256.to_string(),
    }
}

/// One 8-bit grey PNG of a crop, exactly as ocrs read it (design part 10.12, EVID-5). Def: T5-8. Use: T5-8, T5-10.
pub fn grey_png(image: &Grey) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&image.pixels).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}

/// Reads an 8-bit grey PNG back, for arm R' of the trial and the goldens (design parts 10.4, 10.9). Def: T5-8.
/// Use: T5-8, T5-10, T5-11.
pub fn read_png_grey(bytes: &[u8]) -> Result<Grey, String> {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    if info.color_type != png::ColorType::Grayscale || info.bit_depth != png::BitDepth::Eight {
        return Err("not an 8-bit grey PNG".to_string());
    }
    let size = reader.output_buffer_size().ok_or_else(|| "the PNG is too large".to_string())?;
    let mut pixels = vec![0u8; size];
    let frame = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    if frame.color_type != png::ColorType::Grayscale || frame.bit_depth != png::BitDepth::Eight {
        return Err("not an 8-bit grey PNG".to_string());
    }
    pixels.truncate(frame.buffer_size());
    Ok(Grey { width: frame.width, height: frame.height, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::subtitle::bitmap::lines::LineBox;
    use lazymkv_core::subtitle::bitmap::prepare::Grey;

    fn ramp() -> Grey {
        let (width, height) = (7u32, 3u32);
        Grey { width, height, pixels: (0..width * height).map(|i| (i * 12) as u8).collect() }
    }

    #[test]
    fn engine_id_names_ocrs_0_13_1_and_both_pins() {
        let id = engine_id();
        assert_eq!(id.ocrs, "0.13.1");
        assert_eq!(id.detection_sha256, lazymkv_ocr_models::DETECTION_SHA256);
        assert_eq!(id.recognition_sha256, lazymkv_ocr_models::RECOGNITION_SHA256);
        for pin in [&id.detection_sha256, &id.recognition_sha256] {
            assert_eq!(pin.len(), 64, "{pin}");
            assert!(pin.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "{pin}");
        }
    }

    #[test]
    fn ocrs_version_is_the_workspaces_ocrs_pin() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
        let text = std::fs::read_to_string(path).unwrap();
        let line = text.lines().find(|line| line.starts_with("ocrs = ")).expect("the workspace pins ocrs");
        assert!(line.contains(&format!("version = \"={OCRS_VERSION}\"")), "{line}");
    }

    #[test]
    fn grey_png_round_trips_an_8_bit_grey_image() {
        let image = ramp();
        let bytes = grey_png(&image).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        // The IHDR chunk's data starts at byte 16: width, height, then bit depth 8 and colour type 0 (grey).
        assert_eq!((bytes[24], bytes[25]), (8, 0));
        assert_eq!(read_png_grey(&bytes), Ok(image));
    }

    #[test]
    fn read_png_grey_refuses_a_colour_png() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&[0u8; 12]).unwrap();
        }
        assert_eq!(read_png_grey(&bytes), Err("not an 8-bit grey PNG".to_string()));
    }

    #[test]
    fn the_embedded_models_load_and_read_a_white_image() {
        let ocr = Ocr::load().unwrap();
        // Not 64 x 32: ocrs 0.13.1 pads an image smaller than the detector's input with black (detection.rs lines
        // 155 to 160), and a 64 x 32 white block on black is itself detected as a word; 332 x 72 is read as empty.
        let white = Grey { width: 332, height: 72, pixels: vec![255; 332 * 72] };
        let read = ocr.recognize(&white, &[LineBox { x: 0, y: 0, width: 332, height: 72 }]).unwrap();
        assert!(read.len() <= 1, "one box gives at most one line");
        assert!(ocr.recognize(&white, &[]).unwrap().is_empty());
        let detected = ocr.detect(&white).unwrap();
        assert!(detected.boxes.is_empty() && detected.lines.is_empty());
    }
}
