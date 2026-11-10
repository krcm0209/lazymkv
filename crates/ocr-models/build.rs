//! Puts the two pinned models into OUT_DIR for lib.rs's include_bytes!: from the folder LAZYMKV_OCRS_MODELS names
//! when it is set (the mini PC through scripts/box.sh, CI from its model cache), else downloaded from the pinned URLs
//! (CI's models job on a cache miss, and nowhere else). Every file is checked against its SHA-256 pin before it is
//! written; a mismatch, a missing file or a failed download fails the build naming the variable (design part 10.11,
//! calls 2 and 3, A3).

#[path = "src/pins.rs"]
#[allow(dead_code)]
mod pins;

use pins::*;
use std::path::{Path, PathBuf};

fn download(url: &str) -> Result<Vec<u8>, String> {
    println!("cargo:warning=lazymkv-ocr-models: {MODELS_VAR} is unset; downloading {url}");
    let failed = |why: String| {
        format!("lazymkv-ocr-models: {url} could not be downloaded ({why}); set {MODELS_VAR} to a folder holding {DETECTION_NAME} and {RECOGNITION_NAME}")
    };
    let response = ureq::get(url).call().map_err(|error| failed(error.to_string()))?;
    response.into_body().with_config().limit(16 * 1024 * 1024).read_to_vec().map_err(|error| failed(error.to_string()))
}

fn models() -> Result<[Vec<u8>; 2], String> {
    match std::env::var_os(MODELS_VAR) {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            println!("cargo:rerun-if-changed={}", dir.join(DETECTION_NAME).display());
            println!("cargo:rerun-if-changed={}", dir.join(RECOGNITION_NAME).display());
            from_folder(&dir)
        }
        None => {
            let bucket = Path::new("https://ocrs-models.s3-accelerate.amazonaws.com");
            let detection = download(DETECTION_URL)?;
            verified(bucket, DETECTION_NAME, &detection, DETECTION_SHA256)?;
            let recognition = download(RECOGNITION_URL)?;
            verified(bucket, RECOGNITION_NAME, &recognition, RECOGNITION_SHA256)?;
            Ok([detection, recognition])
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/pins.rs");
    println!("cargo:rerun-if-env-changed={MODELS_VAR}");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    match models() {
        Ok([detection, recognition]) => {
            for (name, bytes) in [(DETECTION_NAME, detection), (RECOGNITION_NAME, recognition)] {
                let path = out.join(name);
                if let Err(error) = std::fs::write(&path, bytes) {
                    eprintln!("lazymkv-ocr-models: {} could not be written ({error})", path.display());
                    std::process::exit(1);
                }
            }
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
