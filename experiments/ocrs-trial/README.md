# ocrs-trial

A throw-away phase 0 tool, not part of lazymkv. It reads subtitle frames with the pure-Rust OCR engine
[ocrs](https://github.com/robertknight/ocrs) 0.13.1 so that its output can be set beside Tesseract's on the same
frames (experiment 14). Decision D17 says the app must ship its OCR engine inside the installer; decision D19 says the
subtitle language comes from the track's tag, so ocrs's ASCII-only alphabet may be enough. The trial measures speed on
the mini PC (a Celeron J4125 without AVX2, so rten takes its generic code path) and how well ocrs reads the SDH markers.
The research behind it is in `docs/phase0/ocr-engine-research.md`.

This crate is its own Cargo workspace (the empty `[workspace]` table), so the root workspace does not build it.

## Use

```
ocrs-trial --detection <model file> --recognition <model file> --frames <dir> --out <jsonl> [--limit N]
```

It loads the two models from the files given (no download and no network: the ocrs library never fetches models, only
the ocrs CLI does), reads every `*.png` in `<dir>` in name order (the first N with `--limit`), and writes to `<out>`:

- one line per frame: `{"frame": k, "file": "<name>", "lines": ["...", ...], "ms": <decode + OCR time>}`
- a last line: `{"summary": {"frames", "ms_total", "ms_per_frame", "load_ms", "ocrs", "models": {"detection",
  "recognition"}}}`, where the model values are the SHA-1 of the model files.

Standard output carries progress and timing only, never recognised text. Exit status: 0 done, 1 failed, 2 bad
arguments. The phase 0 job `scripts/phase0/ocrs-trial.ps1` runs it on one subtitle stream of the tuning dataset, and
`scripts/phase0/ocr_compare.py` compares its output with Tesseract's.

Build on the mini PC (release only: debug builds of rten are extremely slow):

```
scripts/box.sh cargo build --release --manifest-path experiments/ocrs-trial/Cargo.toml
```

The exe lands in `C:\lazymkv\target\release\ocrs-trial.exe` (box.sh sets `CARGO_TARGET_DIR`). The direct
dependencies are pinned with `=`; this crate's `Cargo.lock` is written on the mini PC and box.sh brings back only the
root one, so fetch it to commit it:
`scp -q -o BatchMode=yes USER@makemkv-box:C:/lazymkv/src/experiments/ocrs-trial/Cargo.lock experiments/ocrs-trial/`.

## Models (not committed)

The models are NOT in this repository (`experiments/ocrs-trial/models/` is git-ignored for a local copy). The trial
uses the two files that ocrs 0.13.1 itself documents: the defaults of the ocrs CLI (`ocrs-cli/src/main.rs`) and of the
library's `examples/download-models.sh`:

| File | Source | Bytes | SHA-1 |
|---|---|---|---|
| `text-detection.onnx` | <https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx> | 2499479 | `709176816006063da8155f549aa0e44735ed0e9c` |
| `text-recognition.onnx` | <https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx> | 9713177 | `b16f361df884a9afaa067e4d6932567f6f652a81` |

On the mini PC they go in `C:\lazymkv\phase0\models\`.

Licence: the ocrs models are licensed under the Creative Commons Attribution-ShareAlike 4.0 International licence
(CC-BY-SA-4.0, <https://creativecommons.org/licenses/by-sa/4.0/>), as stated on the model card
<https://huggingface.co/robertknight/ocrs>. Attribution: "ocrs pre-trained text detection and recognition models by
Robert Knight (<https://github.com/robertknight/ocrs-models>), licensed under CC-BY-SA-4.0; trained on HierText
(Google Research, CC BY-SA 4.0) and synthetic data." The models are used unmodified. The model card covers the
Hugging Face files; the S3 files above carry no licence of their own, which is one more point for the owner's reading
of CC-BY-SA-4.0 before ocrs could ship. The ocrs code is MIT OR Apache-2.0.
