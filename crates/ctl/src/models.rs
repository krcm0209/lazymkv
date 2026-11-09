//! `lazymkvctl export-models DIR`: writes the two OCR model files this program carries, unmodified, so no
//! technological measure restricts them (design parts 9.6, 11.4; call 16, D22).

use lazymkv_ocr_models::{DETECTION, DETECTION_NAME, RECOGNITION, RECOGNITION_NAME};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

fn failed(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(|error| failed(path, error))?;
    // The file is ours (create_new made it), so a failed write removes the part of it that landed.
    file.write_all(bytes).map_err(|error| {
        drop(file);
        let _ = fs::remove_file(path);
        failed(path, error)
    })
}

/// Writes `DETECTION` to `dir`/text-detection.onnx and `RECOGNITION` to `dir`/text-recognition.onnx, byte for byte,
/// making `dir` first when it is missing; refuses, writing neither, when either file exists (the detection file
/// checked first, T5-R34). Design parts 9.6, 11.4; call 16. Def: T5-23. Use: T5-23, T5-26.
pub fn export_models(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| failed(dir, error))?;
    let detection = dir.join(DETECTION_NAME);
    let recognition = dir.join(RECOGNITION_NAME);
    for target in [&detection, &recognition] {
        if fs::symlink_metadata(target).is_ok() {
            return Err(format!("{} exists; export-models writes neither file over an existing one", target.display()));
        }
    }
    write_new(&detection, DETECTION)?;
    if let Err(message) = write_new(&recognition, RECOGNITION) {
        let _ = fs::remove_file(&detection);
        return Err(message);
    }
    Ok(())
}
