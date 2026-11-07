//! The two pinned OCR models: names, URLs, lengths, SHA-1 and SHA-256 pins, and the check of bytes against them. Shared
//! by build.rs (through #[path]) and the library, so each pin has one copy; uses only std and sha2, which are both the
//! package's normal and build-dependencies (design parts 10.11 and 11.4; notes calls 2 and 3, A3, E3; T5-R36).

use sha2::{Digest, Sha256};
use std::path::Path;

/// The environment variable naming the folder that holds the two models, which the build script reads (design 10.11; call 2). Def: T5-7. Use: T5-7.
pub const MODELS_VAR: &str = "LAZYMKV_OCRS_MODELS";
/// The file name of the text-detection model (design 10.11; call 2). Def: T5-7. Use: T5-7, T5-23.
pub const DETECTION_NAME: &str = "text-detection.onnx";
/// The file name of the text-recognition model (design 10.11; call 2). Def: T5-7. Use: T5-7, T5-23.
pub const RECOGNITION_NAME: &str = "text-recognition.onnx";
/// The pinned download URL of the text-detection model, used only by CI's models job (design 10.11; call 2). Def: T5-7. Use: T5-7.
pub const DETECTION_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";
/// The pinned download URL of the text-recognition model, used only by CI's models job (design 10.11; call 2). Def: T5-7. Use: T5-7.
pub const RECOGNITION_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";
/// The pinned byte length of the text-detection model (design 10.11; call 2, A3). Def: T5-7. Use: T5-7, T5-23.
pub const DETECTION_LEN: usize = 2_499_479;
/// The pinned byte length of the text-recognition model (design 10.11; call 2, A3). Def: T5-7. Use: T5-7, T5-23.
pub const RECOGNITION_LEN: usize = 9_713_177;
/// The SHA-1 of the text-detection model, phase 0's value and A3's provenance (design 10.11; A3). Def: T5-7. Use: T5-7.
pub const DETECTION_SHA1: &str = "709176816006063da8155f549aa0e44735ed0e9c";
/// The SHA-1 of the text-recognition model, phase 0's value and A3's provenance (design 10.11; A3). Def: T5-7. Use: T5-7.
pub const RECOGNITION_SHA1: &str = "b16f361df884a9afaa067e4d6932567f6f652a81";
/// The pinned SHA-256 of the text-detection model (design 10.11, 11.4; call 3, A3). Def: T5-7. Use: T5-7, T5-8, T5-23.
pub const DETECTION_SHA256: &str = "a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe";
/// The pinned SHA-256 of the text-recognition model (design 10.11, 11.4; call 3, A3). Def: T5-7. Use: T5-7, T5-8, T5-23.
pub const RECOGNITION_SHA256: &str = "86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5";

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The length (DETECTION_LEN or RECOGNITION_LEN, chosen by `name`) and then the SHA-256 of `bytes` (design 10.11; call 2, R4-commands-17). Def: T5-7. Use: T5-7.
pub fn verified(dir: &Path, name: &str, bytes: &[u8], want: &str) -> Result<(), String> {
    let want_len = match name {
        DETECTION_NAME => DETECTION_LEN,
        RECOGNITION_NAME => RECOGNITION_LEN,
        _ => return Err(format!("lazymkv-ocr-models: {name} is not a pinned model")),
    };
    let dir = dir.display();
    if bytes.len() != want_len {
        return Err(format!(
            "lazymkv-ocr-models: {name} in {dir} has {} bytes, not {want_len}; point {MODELS_VAR} at a folder holding the pinned models",
            bytes.len()
        ));
    }
    let got = sha256_hex(bytes);
    if got != want {
        return Err(format!(
            "lazymkv-ocr-models: {name} in {dir} has SHA-256 {got}, not {want}; point {MODELS_VAR} at a folder holding the pinned models"
        ));
    }
    Ok(())
}

/// Both files of `dir`, each read and verified, detection first (design 10.11; call 2, R4-commands-17). Def: T5-7. Use: T5-7.
pub fn from_folder(dir: &Path) -> Result<[Vec<u8>; 2], String> {
    let read = |name: &str, want: &str| -> Result<Vec<u8>, String> {
        let bytes = std::fs::read(dir.join(name)).map_err(|why| {
            format!(
                "lazymkv-ocr-models: {name} could not be read from {} ({why}); set {MODELS_VAR} to a folder holding {DETECTION_NAME} and {RECOGNITION_NAME}",
                dir.display()
            )
        })?;
        verified(dir, name, &bytes, want)?;
        Ok(bytes)
    };
    let detection = read(DETECTION_NAME, DETECTION_SHA256)?;
    let recognition = read(RECOGNITION_NAME, RECOGNITION_SHA256)?;
    Ok([detection, recognition])
}
