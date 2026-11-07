//! lazymkv-ocr-models: the two pinned ocrs models, text-detection.onnx and text-recognition.onnx (CC-BY-SA-4.0,
//! Creative Commons Attribution-ShareAlike 4.0), embedded through this package's build script and linked without
//! rten, so lazymkvctl carries them for export-models and licenses (design parts 10.11 and 11.4; notes E3, call 23).

mod pins;
pub use pins::*;

/// The embedded text-detection model, the pinned bytes the build script wrote to OUT_DIR (design 10.11, 11.4; call 2, E3). Def: T5-7. Use: T5-8, T5-23.
pub static DETECTION: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text-detection.onnx"));

/// The embedded text-recognition model, the pinned bytes the build script wrote to OUT_DIR (design 10.11, 11.4; call 2, E3). Def: T5-7. Use: T5-8, T5-23.
pub static RECOGNITION: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text-recognition.onnx"));

/// The attribution text of the two embedded models (CC-BY-SA-4.0, the models card, HierText), which export-models and licenses print (design 11.4; call 23, R4-commands-54). Def: T5-7. Use: T5-23.
pub const ATTRIBUTION: &str = concat!(
    "lazymkv embeds the ocrs project's two optical character recognition (OCR) models, text-detection.onnx and ",
    "text-recognition.onnx, licensed under the Creative Commons Attribution-ShareAlike 4.0 International licence ",
    "(CC-BY-SA-4.0).\n",
    "Source: https://huggingface.co/robertknight/ocrs\n",
    "The models were trained on the HierText dataset (CC BY-SA 4.0) and on synthetic data.\n",
    "lazymkv ships them unmodified; lazymkvctl export-models writes out the embedded bytes as they are.\n",
);

/// The legal code of CC-BY-SA-4.0, byte for byte the approved source (design 11.4; call 23, R4-commands-54). Def: T5-7. Use: T5-23.
pub const LICENCE_TEXT: &str = include_str!("../CC-BY-SA-4.0.txt");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pins::sha256_hex;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// A fresh, empty folder for one test: `<temp>/lazymkv-ocr-models-<test>-<pid>`.
    fn folder(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lazymkv-ocr-models-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_embedded_models_have_their_pinned_lengths_and_sha256() {
        assert_eq!((DETECTION.len(), RECOGNITION.len()), (2_499_479, 9_713_177));
        assert_eq!(DETECTION_LEN + RECOGNITION_LEN, 12_212_656);
        assert_eq!(sha256_hex(DETECTION), DETECTION_SHA256);
        assert_eq!(sha256_hex(RECOGNITION), RECOGNITION_SHA256);
        for pin in [DETECTION_SHA256, RECOGNITION_SHA256] {
            assert_eq!(pin.len(), 64, "{pin}");
            assert!(pin.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "{pin}");
        }
        assert_eq!(MODELS_VAR, "LAZYMKV_OCRS_MODELS");
        assert_eq!(DETECTION_URL, format!("https://ocrs-models.s3-accelerate.amazonaws.com/{DETECTION_NAME}"));
        assert_eq!(RECOGNITION_URL, format!("https://ocrs-models.s3-accelerate.amazonaws.com/{RECOGNITION_NAME}"));
    }

    #[test]
    fn a_folder_holding_the_pinned_models_reads_detection_first() {
        let dir = folder("pinned");
        fs::write(dir.join(DETECTION_NAME), DETECTION).unwrap();
        fs::write(dir.join(RECOGNITION_NAME), RECOGNITION).unwrap();
        let read = from_folder(&dir);
        fs::remove_dir_all(&dir).unwrap();
        let [detection, recognition] = read.unwrap();
        assert!(detection == DETECTION && recognition == RECOGNITION);
    }

    #[test]
    fn a_model_with_one_changed_byte_is_refused_naming_the_variable() {
        let dir = folder("changed");
        let mut changed = DETECTION.to_vec();
        changed[1000] ^= 0x01;
        fs::write(dir.join(DETECTION_NAME), &changed).unwrap();
        fs::write(dir.join(RECOGNITION_NAME), RECOGNITION).unwrap();
        let read = from_folder(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            read.unwrap_err(),
            format!(
                "lazymkv-ocr-models: text-detection.onnx in {} has SHA-256 {}, not {DETECTION_SHA256}; \
                 point LAZYMKV_OCRS_MODELS at a folder holding the pinned models",
                dir.display(),
                sha256_hex(&changed)
            )
        );
    }

    #[test]
    fn a_model_of_the_wrong_length_is_refused_before_its_hash() {
        let dir = Path::new("models-folder-sentinel");
        assert_eq!(
            verified(dir, RECOGNITION_NAME, &RECOGNITION[..100], RECOGNITION_SHA256).unwrap_err(),
            format!(
                "lazymkv-ocr-models: text-recognition.onnx in {} has 100 bytes, not 9713177; \
                 point LAZYMKV_OCRS_MODELS at a folder holding the pinned models",
                dir.display()
            )
        );
        assert_eq!(verified(dir, RECOGNITION_NAME, RECOGNITION, RECOGNITION_SHA256), Ok(()));
    }

    #[test]
    fn a_folder_missing_a_model_is_refused_naming_the_variable() {
        let dir = folder("missing");
        fs::write(dir.join(RECOGNITION_NAME), RECOGNITION).unwrap();
        let read = from_folder(&dir);
        fs::remove_dir_all(&dir).unwrap();
        let message = read.unwrap_err();
        let head = format!("lazymkv-ocr-models: text-detection.onnx could not be read from {} (", dir.display());
        assert!(message.starts_with(&head), "{message}");
        assert!(
            message.ends_with(
                "); set LAZYMKV_OCRS_MODELS to a folder holding text-detection.onnx and text-recognition.onnx"
            ),
            "{message}"
        );
    }

    #[test]
    fn the_attribution_names_the_licence_the_card_and_hiertext() {
        for part in [
            "the ocrs project's", DETECTION_NAME, RECOGNITION_NAME, "(CC-BY-SA-4.0)", "HierText dataset (CC BY-SA 4.0)",
            "synthetic data", "ships them unmodified",
        ] {
            assert!(ATTRIBUTION.contains(part), "{part}");
        }
        assert!(ATTRIBUTION.lines().any(|line| line == "Source: https://huggingface.co/robertknight/ocrs"), "{ATTRIBUTION}");
        assert!(ATTRIBUTION.contains("https://huggingface.co/"), "{ATTRIBUTION}");
        assert!(ATTRIBUTION.ends_with('\n'));
    }

    #[test]
    fn the_licence_text_is_the_approved_cc_by_sa_4_0_legal_code() {
        assert_eq!(
            sha256_hex(LICENCE_TEXT.as_bytes()),
            "28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5"
        );
        assert!(LICENCE_TEXT.contains("Attribution-ShareAlike 4.0 International"));
    }

    #[test]
    fn the_ci_model_cache_is_keyed_by_both_sha256_pins() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/ci.yml");
        let ci = fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert!(ci.contains(&format!("OCRS_DETECTION_SHA256: {DETECTION_SHA256}")), "the detection pin");
        assert!(ci.contains(&format!("OCRS_RECOGNITION_SHA256: {RECOGNITION_SHA256}")), "the recognition pin");
        let key = "key: ocrs-models-${{ env.OCRS_DETECTION_SHA256 }}-${{ env.OCRS_RECOGNITION_SHA256 }}";
        assert_eq!(ci.matches(key).count(), 3, "the models job's restore and save, the test job's restore");
        assert_eq!(ci.matches("path: .ocrs-models").count(), 3);
        assert_eq!(ci.matches("enableCrossOsArchive: true").count(), 3);
        assert_eq!(ci.matches("fail-on-cache-miss: true").count(), 1, "the test job never downloads");
        assert_eq!(ci.matches("LAZYMKV_OCRS_MODELS: ${{ github.workspace }}/.ocrs-models").count(), 1);
    }
}
